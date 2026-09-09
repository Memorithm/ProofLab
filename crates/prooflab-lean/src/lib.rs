//! Explicit process boundary to the configured Lean environment.
//!
//! A successful process result is necessary for kernel acceptance. The higher
//! level [`LeanKernel::verify_job`] path additionally checks formal-source
//! integrity and emits a [`prooflab_core::ProofArtifact`] only after acceptance.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use prooflab_core::{
    FormalBackend, FormalStatement, KernelReceipt, ProofArtifact, ProofArtifactError,
    ProofArtifactId, ReproMeta, sha256_bytes,
};

const LEAN_INVOCATION: &str = "lake env lean";

/// A verification request binding an exact formal statement to a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationJob {
    pub formal_statement: FormalStatement,
    pub source: PathBuf,
    pub dependencies: Vec<ProofArtifactId>,
}

impl VerificationJob {
    /// Create a verification job with normalized proof dependencies.
    #[must_use]
    pub fn new(
        formal_statement: FormalStatement,
        source: impl Into<PathBuf>,
        mut dependencies: Vec<ProofArtifactId>,
    ) -> Self {
        dependencies.sort_unstable();
        dependencies.dedup();
        Self {
            formal_statement,
            source: source.into(),
            dependencies,
        }
    }
}

/// Raw normalized result returned by the Lean process boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelResult {
    pub accepted: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_digest: [u8; 32],
    pub stderr_digest: [u8; 32],
}

/// Result of a verification job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationOutcome {
    pub result: KernelResult,
    pub proof: Option<ProofArtifact>,
}

/// Fail-closed error at the Lean trust boundary.
#[derive(Debug)]
pub enum VerificationError {
    Io(std::io::Error),
    FormalStatementIntegrity,
    SourceDigestMismatch,
    ProofArtifact(ProofArtifactError),
}

impl fmt::Display for VerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Lean verification I/O error: {error}"),
            Self::FormalStatementIntegrity => write!(formatter, "formal statement id mismatch"),
            Self::SourceDigestMismatch => write!(
                formatter,
                "Lean source digest does not match formal statement"
            ),
            Self::ProofArtifact(error) => {
                write!(formatter, "proof artifact construction failed: {error}")
            }
        }
    }
}

impl std::error::Error for VerificationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ProofArtifact(error) => Some(error),
            Self::FormalStatementIntegrity | Self::SourceDigestMismatch => None,
        }
    }
}

impl From<std::io::Error> for VerificationError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProofArtifactError> for VerificationError {
    fn from(error: ProofArtifactError) -> Self {
        Self::ProofArtifact(error)
    }
}

/// Configured Lean command boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeanKernel {
    lake_binary: PathBuf,
}

impl Default for LeanKernel {
    fn default() -> Self {
        Self {
            lake_binary: PathBuf::from("lake"),
        }
    }
}

impl LeanKernel {
    #[must_use]
    pub fn new(lake_binary: impl Into<PathBuf>) -> Self {
        Self {
            lake_binary: lake_binary.into(),
        }
    }

    /// Verify a Lean file through the pinned Lake environment that owns it.
    ///
    /// The source must live below a directory containing `lakefile.lean` or
    /// `lakefile.toml`. The kernel command is executed from that directory so
    /// dependency search paths do not depend on the Rust caller's working directory.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the source cannot be canonicalized, no owning
    /// Lake project can be found, or the configured `lake` executable cannot be
    /// started or its output cannot be collected. A Lean rejection itself is
    /// represented by `KernelResult::accepted == false` and is not an I/O error.
    pub fn verify_file(&self, source: impl AsRef<Path>) -> std::io::Result<KernelResult> {
        let source = source.as_ref().canonicalize()?;
        let project_root = lake_project_root(&source)?;
        let output = Command::new(&self.lake_binary)
            .current_dir(project_root)
            .arg("env")
            .arg("lean")
            .arg(&source)
            .output()?;

        Ok(KernelResult {
            accepted: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            stdout_digest: sha256_bytes(&output.stdout),
            stderr_digest: sha256_bytes(&output.stderr),
        })
    }

    /// Verify a source-bound job and create a proof artifact only on acceptance.
    ///
    /// A rejected Lean process returns `Ok(VerificationOutcome { proof: None, .. })`.
    /// It never becomes a proof artifact.
    ///
    /// # Errors
    ///
    /// Fails before kernel invocation when the formal statement id or source
    /// digest is inconsistent. I/O failures and proof-artifact invariant failures
    /// are also surfaced explicitly.
    pub fn verify_job(
        &self,
        job: &VerificationJob,
        repro: ReproMeta,
    ) -> Result<VerificationOutcome, VerificationError> {
        if !job.formal_statement.check_id() {
            return Err(VerificationError::FormalStatementIntegrity);
        }
        let source_bytes = fs::read(&job.source)?;
        if !job.formal_statement.matches_source(&source_bytes) {
            return Err(VerificationError::SourceDigestMismatch);
        }

        let result = self.verify_file(&job.source)?;
        let proof = if result.accepted {
            let receipt = KernelReceipt::new(
                FormalBackend::Lean4,
                LEAN_INVOCATION,
                true,
                result.exit_code,
                result.stdout_digest,
                result.stderr_digest,
            );
            Some(ProofArtifact::new_verified(
                &job.formal_statement,
                &source_bytes,
                job.dependencies.clone(),
                repro,
                receipt,
            )?)
        } else {
            None
        };
        Ok(VerificationOutcome { result, proof })
    }
}

fn lake_project_root(source: &Path) -> io::Result<PathBuf> {
    let mut directory = source.parent();
    while let Some(candidate) = directory {
        if candidate.join("lakefile.lean").is_file() || candidate.join("lakefile.toml").is_file() {
            return Ok(candidate.to_path_buf());
        }
        directory = candidate.parent();
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "no Lake project root found above source {}",
            source.display()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use prooflab_core::{Claim, ClaimBody, FormalStatement, ReproMeta};

    use super::*;

    #[test]
    fn default_boundary_uses_lake() {
        assert_eq!(LeanKernel::default().lake_binary, PathBuf::from("lake"));
    }

    #[test]
    fn locates_lake_project_from_source_instead_of_process_cwd() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ProofLab/Core/Smoke.lean");
        let source = source.canonicalize().unwrap();
        let root = lake_project_root(&source).unwrap();
        assert!(root.join("lakefile.lean").is_file());
    }

    #[test]
    fn source_mismatch_fails_before_kernel_invocation() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ProofLab/Core/Smoke.lean");
        let claim = Claim::new(ClaimBody {
            statement: "1 + 1 = 2".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let formal = FormalStatement::lean4(claim.id, b"different source", vec![]);
        let job = VerificationJob::new(formal, source, vec![]);
        let mut repro = ReproMeta::bootstrap("test-environment");
        repro.prooflab_revision = "test-revision".into();
        let kernel = LeanKernel::new("this-command-must-not-run");
        assert!(matches!(
            kernel.verify_job(&job, repro),
            Err(VerificationError::SourceDigestMismatch)
        ));
    }
}
