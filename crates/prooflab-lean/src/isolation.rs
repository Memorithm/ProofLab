use std::collections::BTreeSet;
#[cfg(target_os = "linux")]
use std::fs::File;
use std::io;
#[cfg(target_os = "linux")]
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(target_os = "linux")]
use nix::fcntl::{FcntlArg, SealFlag, fcntl};
#[cfg(target_os = "linux")]
use nix::sys::memfd::{MFdFlags, memfd_create};

const BACKEND_VERSION: &str = "linux-setuid-bubblewrap-v2";

/// Kernel-enforced resource budgets applied inside the isolated Lean boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeanIsolationLimits {
    pub max_address_space_bytes: u64,
    pub max_cpu_seconds: u64,
    pub max_processes: u64,
    pub max_file_size_bytes: u64,
    pub max_open_files: u64,
}

impl Default for LeanIsolationLimits {
    fn default() -> Self {
        Self {
            max_address_space_bytes: 2 * 1024 * 1024 * 1024,
            max_cpu_seconds: 120,
            max_processes: 128,
            max_file_size_bytes: 64 * 1024 * 1024,
            max_open_files: 256,
        }
    }
}

impl LeanIsolationLimits {
    fn validate(self) -> io::Result<()> {
        if self.max_address_space_bytes == 0
            || self.max_cpu_seconds == 0
            || self.max_processes == 0
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
            "max_as_bytes={};max_cpu_seconds={};max_processes={};max_file_bytes={};max_open_files={}",
            self.max_address_space_bytes,
            self.max_cpu_seconds,
            self.max_processes,
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
}

impl BubblewrapIsolation {
    #[must_use]
    pub fn new(bubblewrap_binary: impl Into<PathBuf>, prlimit_binary: impl Into<PathBuf>) -> Self {
        Self {
            bubblewrap_binary: bubblewrap_binary.into(),
            prlimit_binary: prlimit_binary.into(),
            runtime_roots: Vec::new(),
            limits: LeanIsolationLimits::default(),
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
    ) -> io::Result<Command> {
        let mut roots = self.validated_runtime_roots()?;
        let project_root = canonical_directory(project_root, "Lake project root")?;
        let bubblewrap_binary = canonical_executable(&self.bubblewrap_binary, "bubblewrap")?;
        validate_setuid_root(&bubblewrap_binary)?;
        let prlimit_binary = canonical_executable(&self.prlimit_binary, "prlimit")?;
        let lake_binary = canonical_executable(lake_binary, "lake")?;
        let source = source.canonicalize()?;
        if !source.is_file() || !source.starts_with(&project_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Lean source must be a regular file inside its Lake project",
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
        let mut command = Command::new(&bubblewrap_binary);
        configure_namespace(&mut command, &project_root, roots);
        attach_sealed_source(&mut command, source_bytes)?;
        configure_limits_and_lean(&mut command, &prlimit_binary, &lake_binary, self.limits);
        Ok(command)
    }
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

    #[test]
    fn isolation_limits_are_bound_into_the_contract() {
        let isolation = BubblewrapIsolation::new("/usr/bin/false", "/usr/bin/false").with_limits(
            LeanIsolationLimits {
                max_address_space_bytes: 10,
                max_cpu_seconds: 11,
                max_processes: 12,
                max_file_size_bytes: 13,
                max_open_files: 14,
            },
        );
        let contract = isolation
            .invocation_contract(Path::new("/usr/bin/false"))
            .unwrap();
        assert!(contract.contains("isolation=linux-setuid-bubblewrap-v2"));
        assert!(contract.contains("network=deny_all"));
        assert!(contract.contains("source=readonly_bind_data"));
        assert!(contract.contains("max_as_bytes=10"));
        assert!(contract.contains("max_open_files=14"));
    }

    #[test]
    fn runtime_roots_are_bound_into_the_contract() {
        let base = BubblewrapIsolation::new("/usr/bin/false", "/usr/bin/false");
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
}
