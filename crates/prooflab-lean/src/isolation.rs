use std::collections::BTreeSet;
use std::fs;
#[cfg(target_os = "linux")]
use std::fs::File;
use std::io;
#[cfg(target_os = "linux")]
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use nix::fcntl::{FcntlArg, SealFlag, fcntl};
#[cfg(target_os = "linux")]
use nix::sys::memfd::{MFdFlags, memfd_create};

const BACKEND_VERSION: &str = "linux-setuid-bubblewrap-cgroup-v4";

#[cfg(target_os = "linux")]
static CGROUP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Aggregate cgroup v2 budgets applied to the complete isolated process tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CgroupV2Policy {
    root: PathBuf,
    pub max_memory_bytes: u64,
    pub max_processes: u64,
    pub cpu_quota_micros: u64,
    pub cpu_period_micros: u64,
}

impl CgroupV2Policy {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            max_memory_bytes: 2 * 1024 * 1024 * 1024,
            max_processes: 128,
            cpu_quota_micros: 100_000,
            cpu_period_micros: 100_000,
        }
    }

    #[must_use]
    pub fn with_limits(
        mut self,
        max_memory_bytes: u64,
        max_processes: u64,
        cpu_quota_micros: u64,
        cpu_period_micros: u64,
    ) -> Self {
        self.max_memory_bytes = max_memory_bytes;
        self.max_processes = max_processes;
        self.cpu_quota_micros = cpu_quota_micros;
        self.cpu_period_micros = cpu_period_micros;
        self
    }

    fn validate(&self) -> io::Result<()> {
        if self.max_memory_bytes == 0
            || self.max_processes == 0
            || self.cpu_quota_micros == 0
            || self.cpu_period_micros == 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cgroup v2 aggregate limits must be non-zero",
            ));
        }
        if !self.root.is_absolute() || self.root == Path::new("/") {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "cgroup v2 delegated root must be an absolute non-root path",
            ));
        }
        Ok(())
    }

    fn contract(&self, shell: &Path) -> String {
        let mut contract = format!(
            "cgroup=v2_delegated;max_aggregate_memory_bytes={};max_aggregate_processes={};cpu_quota_micros={};cpu_period_micros={}",
            self.max_memory_bytes,
            self.max_processes,
            self.cpu_quota_micros,
            self.cpu_period_micros,
        );
        push_path_contract(&mut contract, "cgroup_shell", shell);
        contract
    }
}

/// Input-admission and kernel-enforced budgets for the isolated Lean boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeanIsolationLimits {
    pub max_source_bytes: u64,
    pub max_address_space_bytes: u64,
    pub max_cpu_seconds: u64,
    pub max_processes: u64,
    pub max_lean_threads: u64,
    pub max_file_size_bytes: u64,
    pub max_open_files: u64,
}

impl Default for LeanIsolationLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 8 * 1024 * 1024,
            max_address_space_bytes: 2 * 1024 * 1024 * 1024,
            max_cpu_seconds: 120,
            max_processes: 128,
            max_lean_threads: 1,
            max_file_size_bytes: 64 * 1024 * 1024,
            max_open_files: 256,
        }
    }
}

impl LeanIsolationLimits {
    fn validate(self) -> io::Result<()> {
        if self.max_source_bytes == 0
            || self.max_address_space_bytes == 0
            || self.max_cpu_seconds == 0
            || self.max_processes == 0
            || self.max_lean_threads == 0
            || self.max_file_size_bytes == 0
            || self.max_open_files == 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Lean isolation limits must be non-zero",
            ));
        }
        Ok(())
    }

    fn contract(self) -> String {
        format!(
            "max_source_bytes={};max_as_bytes={};max_cpu_seconds={};max_processes={};max_lean_threads={};max_file_bytes={};max_open_files={}",
            self.max_source_bytes,
            self.max_address_space_bytes,
            self.max_cpu_seconds,
            self.max_processes,
            self.max_lean_threads,
            self.max_file_size_bytes,
            self.max_open_files,
        )
    }
}

/// Linux namespace boundary for generated or third-party Lean sources.
///
/// An administrator-owned setuid Bubblewrap helper supplies mount, PID, IPC,
/// UTS, cgroup and network namespaces.
/// The project and runtime roots are mounted read-only; `/tmp` is the only
/// writable filesystem, except for the read-only staged proof-source mount.
/// `prlimit` applies CPU, memory, process, file-size and descriptor limits before
/// Lake starts. The outer `ProofLab` process guard retains the independent
/// wall-clock and bounded-output controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BubblewrapIsolation {
    bubblewrap_binary: PathBuf,
    prlimit_binary: PathBuf,
    runtime_roots: Vec<PathBuf>,
    limits: LeanIsolationLimits,
    cgroup_v2: Option<CgroupV2Policy>,
}

impl BubblewrapIsolation {
    #[must_use]
    pub fn new(bubblewrap_binary: impl Into<PathBuf>, prlimit_binary: impl Into<PathBuf>) -> Self {
        Self {
            bubblewrap_binary: bubblewrap_binary.into(),
            prlimit_binary: prlimit_binary.into(),
            runtime_roots: Vec::new(),
            limits: LeanIsolationLimits::default(),
            cgroup_v2: None,
        }
    }

    /// Add a read-only runtime tree needed by the exact Lake/Lean toolchain.
    #[must_use]
    pub fn with_runtime_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.runtime_roots.push(root.into());
        self
    }

    #[must_use]
    pub fn with_limits(mut self, limits: LeanIsolationLimits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub const fn limits(&self) -> LeanIsolationLimits {
        self.limits
    }

    /// Require a delegated cgroup v2 subtree for aggregate process-tree budgets.
    #[must_use]
    pub fn with_cgroup_v2(mut self, policy: CgroupV2Policy) -> Self {
        self.cgroup_v2 = Some(policy);
        self
    }

    pub(crate) const fn max_source_bytes(&self) -> u64 {
        self.limits.max_source_bytes
    }

    // Contract construction is not a prerequisite for the direct file API.
    // Both paths must independently apply the same canonical mount admission.
    fn validated_runtime_roots(&self) -> io::Result<BTreeSet<PathBuf>> {
        self.limits.validate()?;
        let mut roots = BTreeSet::new();
        for root in &self.runtime_roots {
            roots.insert(canonical_directory(root, "Lean runtime root")?);
        }
        Ok(roots)
    }

    pub(crate) fn invocation_contract(&self, lake_binary: &Path) -> io::Result<String> {
        let roots = self.validated_runtime_roots()?;
        let cgroup = self.cgroup_policy()?;
        canonical_cgroup_root(&cgroup.root)?;
        let shell = canonical_executable(Path::new("/bin/sh"), "cgroup entry shell")?;
        let bubblewrap = canonical_executable(&self.bubblewrap_binary, "bubblewrap")?;
        let prlimit = canonical_executable(&self.prlimit_binary, "prlimit")?;
        let lake = canonical_executable(lake_binary, "lake")?;
        let mut contract = format!(
            "isolation={BACKEND_VERSION};network=deny_all;filesystem=project_ro_runtime_ro_tmpfs;source=readonly_bind_data;{}",
            self.limits.contract()
        );
        push_path_contract(&mut contract, "bubblewrap", &bubblewrap);
        push_path_contract(&mut contract, "prlimit", &prlimit);
        push_path_contract(&mut contract, "lake", &lake);
        contract.push(';');
        contract.push_str(&cgroup.contract(&shell));
        for root in roots {
            push_path_contract(&mut contract, "runtime_root", &root);
        }
        Ok(contract)
    }

    pub(crate) fn command(
        &self,
        lake_binary: &Path,
        project_root: &Path,
        source: &Path,
        source_bytes: &[u8],
    ) -> io::Result<PreparedIsolationCommand> {
        let mut roots = self.validated_runtime_roots()?;
        let project_root = canonical_directory(project_root, "Lake project root")?;
        let bubblewrap_binary = canonical_executable(&self.bubblewrap_binary, "bubblewrap")?;
        validate_setuid_root(&bubblewrap_binary)?;
        let prlimit_binary = canonical_executable(&self.prlimit_binary, "prlimit")?;
        let lake_binary = canonical_executable(lake_binary, "lake")?;
        let shell = canonical_executable(Path::new("/bin/sh"), "cgroup entry shell")?;
        let source = source.canonicalize()?;
        if !source.is_file() || !source.starts_with(&project_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Lean source must be a regular file inside its Lake project",
            ));
        }
        let source_len = u64::try_from(source_bytes.len()).unwrap_or(u64::MAX);
        if source_len > self.limits.max_source_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "isolated Lean source exceeds max_source_bytes={} (observed {source_len})",
                    self.limits.max_source_bytes
                ),
            ));
        }

        roots.insert(project_root.clone());
        lake_binary.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Lake binary has no parent")
        })?;
        if !roots.iter().any(|root| lake_binary.starts_with(root))
            && !lake_binary.starts_with("/usr")
            && !lake_binary.starts_with("/bin")
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "Lake binary {} is outside the declared read-only runtime roots",
                    lake_binary.display()
                ),
            ));
        }
        let cgroup_policy = self.cgroup_policy()?;
        let cgroup = CgroupGuard::create(cgroup_policy)?;
        let mut command = Command::new(shell);
        command
            .arg("-c")
            .arg("printf '%s\\n' \"$$\" > \"$1/cgroup.procs\" || exit 125; shift; exec \"$@\"")
            .arg("prooflab-cgroup-enter")
            .arg(cgroup.path())
            .arg(&bubblewrap_binary);
        configure_namespace(&mut command, &project_root, roots);
        attach_sealed_source(&mut command, source_bytes)?;
        configure_limits_and_lean(&mut command, &prlimit_binary, &lake_binary, self.limits);
        Ok(PreparedIsolationCommand { command, cgroup })
    }

    fn cgroup_policy(&self) -> io::Result<&CgroupV2Policy> {
        let policy = self.cgroup_v2.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "untrusted Lean verification requires an aggregate cgroup v2 policy",
            )
        })?;
        policy.validate()?;
        Ok(policy)
    }
}

#[derive(Debug)]
pub(crate) struct PreparedIsolationCommand {
    command: Command,
    cgroup: CgroupGuard,
}

impl PreparedIsolationCommand {
    pub(crate) fn command_mut(&mut self) -> &mut Command {
        &mut self.command
    }

    pub(crate) fn cleanup(&mut self) -> io::Result<()> {
        self.cgroup.cleanup()
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct CgroupGuard {
    path: PathBuf,
    cleaned: bool,
}

#[cfg(target_os = "linux")]
impl CgroupGuard {
    fn create(policy: &CgroupV2Policy) -> io::Result<Self> {
        let root = canonical_cgroup_root(&policy.root)?;
        validate_cgroup_controllers(&root)?;
        let mut path = None;
        for _ in 0..32 {
            let sequence = CGROUP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate = root.join(format!(
                "prooflab-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => {
                    path = Some(candidate);
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        let path = path.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "failed to allocate a unique ProofLab cgroup",
            )
        })?;
        let mut guard = Self {
            path,
            cleaned: false,
        };
        let configured = (|| {
            write_and_verify(&guard.path.join("memory.max"), &policy.max_memory_bytes.to_string())?;
            if guard.path.join("memory.swap.max").exists() {
                write_and_verify(&guard.path.join("memory.swap.max"), "0")?;
            }
            if guard.path.join("memory.oom.group").exists() {
                write_and_verify(&guard.path.join("memory.oom.group"), "1")?;
            }
            write_and_verify(&guard.path.join("pids.max"), &policy.max_processes.to_string())?;
            write_and_verify(
                &guard.path.join("cpu.max"),
                &format!("{} {}", policy.cpu_quota_micros, policy.cpu_period_micros),
            )?;
            for required in ["cgroup.procs", "cgroup.events", "cgroup.kill"] {
                if !guard.path.join(required).is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        format!("delegated cgroup v2 is missing {required}"),
                    ));
                }
            }
            Ok(())
        })();
        if let Err(error) = configured {
            let _ = guard.cleanup();
            return Err(error);
        }
        Ok(guard)
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn cleanup(&mut self) -> io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        let initial_events = fs::read_to_string(self.path.join("cgroup.events"))?;
        if initial_events.lines().any(|line| line == "populated 1")
            && self.path.join("cgroup.kill").exists()
        {
            fs::write(self.path.join("cgroup.kill"), b"1\n")?;
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let events = fs::read_to_string(self.path.join("cgroup.events"))?;
            if events.lines().any(|line| line == "populated 0") {
                break;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "aggregate cgroup remained populated after bounded cleanup",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
        fs::remove_dir(&self.path)?;
        self.cleaned = true;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for CgroupGuard {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
struct CgroupGuard;

#[cfg(not(target_os = "linux"))]
impl CgroupGuard {
    fn create(_policy: &CgroupV2Policy) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "aggregate cgroup v2 isolation requires Linux",
        ))
    }

    fn path(&self) -> &Path {
        Path::new("")
    }

    fn cleanup(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn canonical_cgroup_root(path: &Path) -> io::Result<PathBuf> {
    let path = canonical_directory(path, "cgroup v2 delegated root")?;
    if path == Path::new("/sys/fs/cgroup") {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the cgroup v2 filesystem root cannot be used directly",
        ));
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn validate_cgroup_controllers(root: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let mountinfo = fs::read_to_string("/proc/self/mountinfo")?;
    let on_cgroup_v2 = mountinfo.lines().any(|line| {
        let Some((mount, filesystem)) = line.split_once(" - ") else {
            return false;
        };
        let Some(mount_point) = mount.split_whitespace().nth(4) else {
            return false;
        };
        filesystem.split_whitespace().next() == Some("cgroup2")
            && root.starts_with(Path::new(mount_point))
    });
    if !on_cgroup_v2 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "configured aggregate resource root is not on a cgroup v2 filesystem",
        ));
    }
    let metadata = root.metadata()?;
    if metadata.mode() & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "cgroup v2 delegated root must not be group- or world-writable",
        ));
    }
    let available = fs::read_to_string(root.join("cgroup.controllers"))?;
    let enabled = fs::read_to_string(root.join("cgroup.subtree_control"))?;
    for controller in ["cpu", "memory", "pids"] {
        if !available.split_whitespace().any(|item| item == controller)
            || !enabled.split_whitespace().any(|item| item == controller)
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("delegated cgroup v2 lacks enabled {controller} controller"),
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn validate_cgroup_controllers(_root: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "aggregate cgroup v2 isolation requires Linux",
    ))
}

#[cfg(target_os = "linux")]
fn write_and_verify(path: &Path, value: &str) -> io::Result<()> {
    fs::write(path, format!("{value}\n"))?;
    let observed = fs::read_to_string(path)?;
    if observed.trim() != value {
        return Err(io::Error::other(format!(
            "cgroup limit {} read back as {:?}, expected {value:?}",
            path.display(),
            observed.trim()
        )));
    }
    Ok(())
}

fn configure_namespace(command: &mut Command, project_root: &Path, roots: BTreeSet<PathBuf>) {
    command
        .arg("--die-with-parent")
        .arg("--new-session")
        .arg("--unshare-all")
        .arg("--unshare-user")
        .arg("--cap-drop")
        .arg("ALL")
        .arg("--proc")
        .arg("/proc")
        .arg("--dev")
        .arg("/dev")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--ro-bind")
        .arg("/usr")
        .arg("/usr");
    for system_root in ["/bin", "/lib", "/lib64", "/sbin"] {
        if Path::new(system_root).exists() {
            command.arg("--ro-bind").arg(system_root).arg(system_root);
        }
    }
    for root in roots {
        command.arg("--ro-bind").arg(&root).arg(&root);
    }
    command
        .arg("--chdir")
        .arg(project_root)
        .arg("--clearenv")
        .arg("--setenv")
        .arg("PATH")
        .arg("/usr/bin:/bin")
        .arg("--setenv")
        .arg("HOME")
        .arg("/tmp")
        .arg("--setenv")
        .arg("TMPDIR")
        .arg("/tmp");
}

fn configure_limits_and_lean(
    command: &mut Command,
    prlimit_binary: &Path,
    lake_binary: &Path,
    limits: LeanIsolationLimits,
) {
    command
        .arg("--")
        .arg(prlimit_binary)
        .arg(format!(
            "--as={}:{}",
            limits.max_address_space_bytes, limits.max_address_space_bytes
        ))
        .arg(format!(
            "--cpu={}:{}",
            limits.max_cpu_seconds, limits.max_cpu_seconds
        ))
        .arg(format!(
            "--nproc={}:{}",
            limits.max_processes, limits.max_processes
        ))
        .arg(format!(
            "--fsize={}:{}",
            limits.max_file_size_bytes, limits.max_file_size_bytes
        ))
        .arg(format!(
            "--nofile={}:{}",
            limits.max_open_files, limits.max_open_files
        ))
        .arg("--")
        .arg(lake_binary)
        .arg("env")
        .arg("lean")
        .arg("-j")
        .arg(limits.max_lean_threads.to_string())
        .arg("/tmp/prooflab-source.lean");
}

fn push_path_contract(contract: &mut String, label: &str, path: &Path) {
    let value = path.as_os_str().as_encoded_bytes();
    contract.push(';');
    contract.push_str(label);
    contract.push('=');
    contract.push_str(&value.len().to_string());
    contract.push(':');
    contract.push_str(&String::from_utf8_lossy(value));
}

#[cfg(target_os = "linux")]
fn attach_sealed_source(command: &mut Command, source_bytes: &[u8]) -> io::Result<()> {
    let descriptor = memfd_create("prooflab-lean-source", MFdFlags::MFD_ALLOW_SEALING)
        .map_err(io::Error::other)?;
    let mut source = File::from(descriptor);
    source.write_all(source_bytes)?;
    source.flush()?;
    source.rewind()?;
    let seals = SealFlag::F_SEAL_SEAL
        | SealFlag::F_SEAL_SHRINK
        | SealFlag::F_SEAL_GROW
        | SealFlag::F_SEAL_WRITE;
    fcntl(&source, FcntlArg::F_ADD_SEALS(seals)).map_err(io::Error::other)?;
    // --file would copy the sealed bytes to a writable file inside /tmp.
    // Keep the destination read-only too, including against unlink/rename.
    command
        .arg("--ro-bind-data")
        .arg("0")
        .arg("/tmp/prooflab-source.lean")
        .stdin(Stdio::from(source));
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn attach_sealed_source(_command: &mut Command, _source_bytes: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "sealed untrusted Lean source staging requires Linux memfd",
    ))
}

fn canonical_executable(path: &Path, label: &str) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("isolated {label} executable must use an absolute path"),
        ));
    }
    let path = path.canonicalize()?;
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("isolated {label} executable is not a regular file"),
        ));
    }
    Ok(path)
}

#[cfg(unix)]
fn validate_setuid_root(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = path.metadata()?;
    if metadata.uid() != 0 || metadata.mode() & 0o4000 == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "isolated Bubblewrap must be an administrator-owned setuid-root helper",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_setuid_root(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "isolated Bubblewrap requires Unix setuid metadata",
    ))
}

fn canonical_directory(path: &Path, label: &str) -> io::Result<PathBuf> {
    let path = path.canonicalize()?;
    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is not a directory"),
        ));
    }
    if path == Path::new("/") {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("the filesystem root cannot be exposed as a {label}"),
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract_cgroup() -> CgroupV2Policy {
        let root = std::env::temp_dir().join(format!(
            "prooflab-cgroup-contract-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        CgroupV2Policy::new(root).with_limits(16 * 1024 * 1024, 7, 50_000, 100_000)
    }

    #[test]
    fn isolation_limits_are_bound_into_the_contract() {
        let isolation = BubblewrapIsolation::new("/usr/bin/false", "/usr/bin/false")
            .with_limits(LeanIsolationLimits {
                max_source_bytes: 9,
                max_address_space_bytes: 10,
                max_cpu_seconds: 11,
                max_processes: 12,
                max_lean_threads: 13,
                max_file_size_bytes: 14,
                max_open_files: 15,
            })
            .with_cgroup_v2(contract_cgroup());
        let contract = isolation
            .invocation_contract(Path::new("/usr/bin/false"))
            .unwrap();
        assert!(contract.contains("isolation=linux-setuid-bubblewrap-cgroup-v4"));
        assert!(contract.contains("network=deny_all"));
        assert!(contract.contains("source=readonly_bind_data"));
        assert!(contract.contains("max_source_bytes=9"));
        assert!(contract.contains("max_as_bytes=10"));
        assert!(contract.contains("max_lean_threads=13"));
        assert!(contract.contains("max_open_files=15"));
        assert!(contract.contains("cgroup=v2_delegated"));
        assert!(contract.contains("max_aggregate_memory_bytes=16777216"));
        assert!(contract.contains("max_aggregate_processes=7"));
        assert!(contract.contains("cpu_quota_micros=50000"));
    }

    #[test]
    fn isolated_lean_worker_count_is_bounded() {
        let mut command = Command::new("/usr/bin/false");
        configure_limits_and_lean(
            &mut command,
            Path::new("/usr/bin/prlimit"),
            Path::new("/usr/bin/lake"),
            LeanIsolationLimits::default(),
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(args.windows(3).any(|window| window == ["lean", "-j", "1"]));
    }

    #[test]
    fn runtime_roots_are_bound_into_the_contract() {
        let base = BubblewrapIsolation::new("/usr/bin/false", "/usr/bin/false")
            .with_cgroup_v2(contract_cgroup());
        let usr = base.clone().with_runtime_root("/usr");
        let tmp = base.with_runtime_root("/tmp");
        assert_ne!(
            usr.invocation_contract(Path::new("/usr/bin/false"))
                .unwrap(),
            tmp.invocation_contract(Path::new("/usr/bin/false"))
                .unwrap()
        );
    }

    #[test]
    fn filesystem_root_is_rejected_as_a_runtime_root() {
        let isolation =
            BubblewrapIsolation::new("/usr/bin/false", "/usr/bin/false").with_runtime_root("/");
        assert_eq!(
            isolation
                .invocation_contract(Path::new("/usr/bin/false"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn launch_rejects_root_and_alias_before_executable_resolution() {
        for root in ["/", "/usr/.."] {
            let isolation = BubblewrapIsolation::new("missing-bwrap", "missing-prlimit")
                .with_runtime_root(root);
            let error = isolation
                .command(
                    Path::new("missing-lake"),
                    Path::new("/tmp"),
                    Path::new("missing-source"),
                    b"theorem input : True := trivial\n",
                )
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("filesystem root"));
            assert_eq!(
                isolation
                    .invocation_contract(Path::new("missing-lake"))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
        }
    }

    #[test]
    fn launch_rejects_filesystem_root_as_project() {
        let isolation = BubblewrapIsolation::new("missing-bwrap", "missing-prlimit");
        let error = isolation
            .command(
                Path::new("missing-lake"),
                Path::new("/"),
                Path::new("missing-source"),
                b"",
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("Lake project root"));
    }

    #[test]
    fn zero_isolation_limit_fails_closed() {
        let limits = LeanIsolationLimits {
            max_processes: 0,
            ..LeanIsolationLimits::default()
        };
        assert_eq!(
            limits.validate().unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        let isolation =
            BubblewrapIsolation::new("missing-bwrap", "missing-prlimit").with_limits(limits);
        assert_eq!(
            isolation
                .invocation_contract(Path::new("missing-lake"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            isolation
                .command(
                    Path::new("missing-lake"),
                    Path::new("/tmp"),
                    Path::new("missing-source"),
                    b"",
                )
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn missing_aggregate_cgroup_policy_fails_closed() {
        let isolation = BubblewrapIsolation::new("/usr/bin/false", "/usr/bin/false");
        let error = isolation
            .invocation_contract(Path::new("/usr/bin/false"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("aggregate cgroup v2"));
    }

    #[test]
    fn zero_aggregate_cgroup_limit_fails_closed() {
        let policy = CgroupV2Policy::new("/tmp/prooflab-cgroup").with_limits(
            0,
            8,
            50_000,
            100_000,
        );
        assert_eq!(policy.validate().unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
}
