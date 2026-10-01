use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const BACKEND_VERSION: &str = "linux-unshare-bubblewrap-v1";

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
/// `unshare` supplies an empty network namespace; Bubblewrap supplies mount,
/// PID, IPC, UTS and cgroup namespaces inside it.
/// The project and runtime roots are mounted read-only; `/tmp` is the only
/// writable filesystem. `prlimit` applies CPU, memory, process, file-size and
/// descriptor limits before Lake starts. The outer `ProofLab` process guard
/// retains the independent wall-clock and bounded-output controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BubblewrapIsolation {
    unshare_binary: PathBuf,
    bubblewrap_binary: PathBuf,
    prlimit_binary: PathBuf,
    runtime_roots: Vec<PathBuf>,
    limits: LeanIsolationLimits,
}

impl BubblewrapIsolation {
    #[must_use]
    pub fn new(
        unshare_binary: impl Into<PathBuf>,
        bubblewrap_binary: impl Into<PathBuf>,
        prlimit_binary: impl Into<PathBuf>,
    ) -> Self {
        Self {
            unshare_binary: unshare_binary.into(),
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

    pub(crate) fn invocation_contract(&self) -> String {
        format!(
            "isolation={BACKEND_VERSION};network=deny_all;filesystem=project_ro_runtime_ro_tmpfs;{}",
            self.limits.contract()
        )
    }

    pub(crate) fn command(
        &self,
        lake_binary: &Path,
        project_root: &Path,
        source: &Path,
    ) -> io::Result<Command> {
        self.limits.validate()?;
        let unshare_binary = canonical_executable(&self.unshare_binary, "unshare")?;
        let bubblewrap_binary = canonical_executable(&self.bubblewrap_binary, "bubblewrap")?;
        let prlimit_binary = canonical_executable(&self.prlimit_binary, "prlimit")?;
        let lake_binary = canonical_executable(lake_binary, "lake")?;
        let project_root = canonical_directory(project_root, "Lake project root")?;
        let source = source.canonicalize()?;
        if !source.is_file() || !source.starts_with(&project_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Lean source must be a regular file inside its Lake project",
            ));
        }

        let mut roots = BTreeSet::new();
        roots.insert(project_root.clone());
        for root in &self.runtime_roots {
            roots.insert(canonical_directory(root, "Lean runtime root")?);
        }
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
        let mut command = Command::new(unshare_binary);
        configure_namespace(&mut command, &bubblewrap_binary, &project_root, roots);
        configure_limits_and_lean(
            &mut command,
            &prlimit_binary,
            &lake_binary,
            &source,
            self.limits,
        );
        Ok(command)
    }
}

fn configure_namespace(
    command: &mut Command,
    bubblewrap_binary: &Path,
    project_root: &Path,
    roots: BTreeSet<PathBuf>,
) {
    command
        .arg("--user")
        .arg("--map-root-user")
        .arg("--net")
        .arg("--")
        .arg(bubblewrap_binary)
        .arg("--die-with-parent")
        .arg("--new-session")
        .arg("--unshare-all")
        .arg("--share-net")
        .arg("--disable-userns")
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
    source: &Path,
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
        .arg(source);
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

fn canonical_directory(path: &Path, label: &str) -> io::Result<PathBuf> {
    let path = path.canonicalize()?;
    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is not a directory"),
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolation_limits_are_bound_into_the_contract() {
        let isolation =
            BubblewrapIsolation::new("/usr/bin/unshare", "/usr/bin/bwrap", "/usr/bin/prlimit")
                .with_limits(LeanIsolationLimits {
                    max_address_space_bytes: 10,
                    max_cpu_seconds: 11,
                    max_processes: 12,
                    max_file_size_bytes: 13,
                    max_open_files: 14,
                });
        let contract = isolation.invocation_contract();
        assert!(contract.contains("isolation=linux-unshare-bubblewrap-v1"));
        assert!(contract.contains("network=deny_all"));
        assert!(contract.contains("max_as_bytes=10"));
        assert!(contract.contains("max_open_files=14"));
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
    }
}
