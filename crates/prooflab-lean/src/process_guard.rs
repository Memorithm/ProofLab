use std::io::{self, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use nix::errno::Errno;
#[cfg(unix)]
use nix::sys::signal::{Signal, killpg};
#[cfg(unix)]
use nix::unistd::Pid;
#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};

use super::{LeanProcessLimits, ProcessTermination};

pub(crate) struct GuardedOutput {
    pub status: ExitStatus,
    pub termination: ProcessTermination,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Stdout,
    Stderr,
}

struct Capture {
    stream: Stream,
    bytes: Vec<u8>,
    limit_exceeded: bool,
    read_error: Option<String>,
}

pub(crate) fn run_command(
    command: &mut Command,
    limits: LeanProcessLimits,
) -> io::Result<GuardedOutput> {
    limits.validate()?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn()?;
    #[cfg(unix)]
    let process_group = Pid::from_raw(pid_to_i32(child.id())?);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Lean stdout pipe was not created"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("Lean stderr pipe was not created"))?;
    let (capture_tx, capture_rx) = mpsc::channel();
    spawn_capture(
        Stream::Stdout,
        stdout,
        limits.max_stdout_bytes,
        capture_tx.clone(),
    );
    spawn_capture(
        Stream::Stderr,
        stderr,
        limits.max_stderr_bytes,
        capture_tx,
    );

    let started = Instant::now();
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }
        if started.elapsed() >= limits.timeout {
            #[cfg(unix)]
            terminate_and_confirm(process_group, limits.termination_grace)?;
            #[cfg(not(unix))]
            terminate_child(&mut child)?;
            break (child.wait()?, true);
        }
        thread::sleep(
            Duration::from_millis(10).min(limits.timeout.saturating_sub(started.elapsed())),
        );
    };

    #[cfg(unix)]
    if !timed_out {
        terminate_and_confirm(process_group, limits.termination_grace)?;
    }

    let (stdout, stderr) = collect_captures(capture_rx, limits.drain_timeout)?;
    let termination = if timed_out {
        ProcessTermination::TimedOut {
            elapsed_ms: duration_millis(started.elapsed()),
        }
    } else if status.code().is_some() {
        ProcessTermination::Exited
    } else {
        ProcessTermination::Signaled {
            signal: exit_signal(&status),
        }
    };

    Ok(GuardedOutput {
        status,
        termination,
        stdout,
        stderr,
    })
}

fn spawn_capture(
    stream: Stream,
    mut reader: impl Read + Send + 'static,
    limit: usize,
    sender: Sender<Capture>,
) {
    let _capture_worker = thread::spawn(move || {
        let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
        let mut buffer = [0_u8; 8 * 1024];
        let mut limit_exceeded = false;
        let mut read_error = None;
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    let remaining = limit.saturating_sub(bytes.len());
                    let retained = remaining.min(read);
                    bytes.extend_from_slice(&buffer[..retained]);
                    if retained < read {
                        limit_exceeded = true;
                    }
                }
                Err(error) => {
                    read_error = Some(error.to_string());
                    break;
                }
            }
        }
        let _ = sender.send(Capture {
            stream,
            bytes,
            limit_exceeded,
            read_error,
        });
    });
}

fn collect_captures(
    receiver: Receiver<Capture>,
    drain_timeout: Duration,
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let deadline = Instant::now()
        .checked_add(drain_timeout)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid drain deadline"))?;
    let mut stdout = None;
    let mut stderr = None;
    for _ in 0..2 {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let capture = receiver.recv_timeout(remaining).map_err(|error| match error {
            RecvTimeoutError::Timeout => {
                io::Error::new(io::ErrorKind::TimedOut, "Lean output drain deadline exceeded")
            }
            RecvTimeoutError::Disconnected => {
                io::Error::other("Lean output capture worker disconnected")
            }
        })?;
        if let Some(error) = capture.read_error {
            return Err(io::Error::other(format!(
                "Lean {:?} capture failed: {error}",
                capture.stream
            )));
        }
        if capture.limit_exceeded {
            return Err(io::Error::other(format!(
                "Lean {:?} exceeded its configured output limit",
                capture.stream
            )));
        }
        match capture.stream {
            Stream::Stdout => stdout = Some(capture.bytes),
            Stream::Stderr => stderr = Some(capture.bytes),
        }
    }
    Ok((
        stdout.ok_or_else(|| io::Error::other("Lean stdout capture is missing"))?,
        stderr.ok_or_else(|| io::Error::other("Lean stderr capture is missing"))?,
    ))
}

#[cfg(unix)]
fn terminate_and_confirm(process_group: Pid, grace: Duration) -> io::Result<()> {
    match killpg(process_group, Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => {
            return Err(io::Error::other(format!(
                "cannot terminate Lean process group: {error}"
            )));
        }
    }

    let deadline = Instant::now().checked_add(grace).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "invalid termination deadline")
    })?;
    loop {
        match killpg(process_group, Option::<Signal>::None) {
            Err(Errno::ESRCH) => return Ok(()),
            Ok(()) => {
                #[cfg(target_os = "linux")]
                if !linux_process_group_has_live_members(process_group.as_raw())? {
                    return Ok(());
                }
            }
            Err(error) => {
                return Err(io::Error::other(format!(
                    "cannot confirm Lean process-group termination: {error}"
                )));
            }
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Lean process group remained live after SIGKILL",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
fn linux_process_group_has_live_members(process_group: i32) -> io::Result<bool> {
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let stat_path = entry.path().join("stat");
        let stat = match std::fs::read_to_string(&stat_path) {
            Ok(stat) => stat,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let (_, fields) = stat.rsplit_once(") ").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid process stat record: {}", stat_path.display()),
            )
        })?;
        let mut fields = fields.split_whitespace();
        let state = fields.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing process state")
        })?;
        let _parent = fields.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing parent pid")
        })?;
        let member_group = fields
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing process group"))?
            .parse::<i32>()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if member_group == process_group && state != "Z" && state != "X" {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(not(unix))]
fn terminate_child(child: &mut std::process::Child) -> io::Result<()> {
    match child.kill() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

#[cfg(unix)]
fn pid_to_i32(pid: u32) -> io::Result<i32> {
    i32::try_from(pid).map_err(|_| io::Error::other("child pid does not fit i32"))
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
