//! Explicit process boundary to the configured Lean environment.
//!
//! A successful process result is necessary for kernel acceptance. The higher
//! level [`LeanKernel::verify_job`] path additionally checks the content-addressed
//! core verification contract, formal-source integrity, and emits a
//! [`prooflab_core::ProofArtifact`] only after acceptance.
//!
//! The PL-2.0 evidence-chain path [`LeanKernel::verify_obligation`] classifies the
//! Lean process into a typed [`prooflab_core::KernelResult`] /
//! [`prooflab_core::KernelOutcome`] (`Accepted` / `Rejected` / `Unknown` /
//! `Timeout`) and seals a proof artifact only via
//! [`prooflab_core::AcceptedKernel`]. Empirical evidence never enters this path.
//!
//! [`LeanKernel::reproduce`] re-checks a stored artifact against an observed
//! environment lock and may re-invoke Lean under that locked contract.
//! Reproduction success is not a `PROVED` authorization; only
//! [`prooflab_core::ProofArtifact::new_verified`] (directly or via
//! [`prooflab_core::AcceptedKernel`]) seals proof status.

#![forbid(unsafe_code)]

mod corpus;
mod false_conjectures;
mod isolation;
mod minimize;
mod process_guard;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub use prooflab_core::VerificationJob;

pub use corpus::{
    CorpusEntry, CorpusError, CorpusPrepared, CorpusRunReport, ExpectedOutcome,
    KNOWN_THEOREM_CORPUS, prepare_corpus, verify_corpus,
};
pub use false_conjectures::{
    CONTROLLED_FALSE_CONJECTURES, FalseConjectureEntry, FalseConjectureError,
    FalseConjectureReport, run_controlled_false_conjecture_battery,
};
pub use isolation::{BubblewrapIsolation, LeanIsolationLimits};
pub use minimize::{
    ASSUMPTION_MINIMIZATION_CORPUS, ExpectedRemovalOutcome, MinimizationEntry, MinimizationError,
    MinimizationPrepared, MinimizationRunReport, PreparedRemovalTrial, RemovalCandidate,
    RemovalOutcome, RemovalTrial, RemovalTrialReport, leave_one_out_candidates,
    prepare_minimization_corpus, verify_assumption_minimization,
};
use prooflab_core::{
    DriftReport, EnvironmentLock, EvidenceError, FormalBackend, FormalStatement, KernelOutcome,
    KernelReceipt, KernelResult, ProofArtifact, ProofArtifactError, ProofArtifactId,
    ProofObligation, ReproduceError as CoreReproduceError, ReproduceOk,
    reproduce as core_reproduce, sha256_bytes,
};

const LEAN_COMMAND: &str = "lake env lean";
pub(crate) const DEFAULT_LEAN_INVOCATION: &str = "lake env lean;supervisor=unix-v1;timeout_ns=120000000000;drain_timeout_ns=1000000000;termination_grace_ns=1000000000;max_stdout_bytes=1048576;max_stderr_bytes=1048576";

/// Resource limits for one Lean kernel process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeanProcessLimits {
    pub timeout: Duration,
    pub drain_timeout: Duration,
    pub termination_grace: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for LeanProcessLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            drain_timeout: Duration::from_secs(1),
            termination_grace: Duration::from_secs(1),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
        }
    }
}

impl LeanProcessLimits {
    fn validate(self) -> io::Result<()> {
        if self.timeout.is_zero()
            || self.drain_timeout.is_zero()
            || self.termination_grace.is_zero()
            || self.max_stdout_bytes == 0
            || self.max_stderr_bytes == 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Lean process limits must be non-zero",
            ));
        }
        Ok(())
    }

    fn invocation_contract(self) -> String {
        format!(
            "{LEAN_COMMAND};supervisor=unix-v1;timeout_ns={};drain_timeout_ns={};termination_grace_ns={};max_stdout_bytes={};max_stderr_bytes={}",
            self.timeout.as_nanos(),
            self.drain_timeout.as_nanos(),
            self.termination_grace.as_nanos(),
            self.max_stdout_bytes,
            self.max_stderr_bytes,
        )
    }
}

/// Why the supervised Lean entrypoint stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessTermination {
    Exited,
    TimedOut { elapsed_ms: u64 },
    Signaled { signal: Option<i32> },
}

/// Raw normalized result returned by the Lean process boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeanProcessResult {
    pub accepted: bool,
    pub exit_code: Option<i32>,
    pub termination: ProcessTermination,
    pub limits: LeanProcessLimits,
    pub stdout: String,
    pub stderr: String,
    pub stdout_digest: [u8; 32],
    pub stderr_digest: [u8; 32],
}

/// Result of a verification job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationOutcome {
    pub result: LeanProcessResult,
    pub proof: Option<ProofArtifact>,
}

/// PL-2.0 evidence-chain verification result.
///
/// `kernel_result` is the typed content-addressed outcome bound to a
/// [`ProofObligation`]. `proof` is present only when that outcome was accepted
/// and sealed through [`prooflab_core::AcceptedKernel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceVerificationOutcome {
    pub process: LeanProcessResult,
    pub kernel_result: KernelResult,
    pub proof: Option<ProofArtifact>,
}

/// Successful kernel-backed reproduce outcome.
///
/// `environment` confirms integrity + lock binding. `reverified` is present only
/// when Lean accepted again under the locked contract. A new acceptance still
/// goes through [`ProofArtifact::new_verified`]; reproduce itself never mutates
/// claim status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReproduceOutcome {
    pub environment: ReproduceOk,
    pub result: LeanProcessResult,
    pub reverified: Option<ProofArtifact>,
}

/// Fail-closed error at the Lean trust boundary.
#[derive(Debug)]
pub enum VerificationError {
    Io(std::io::Error),
    VerificationJobIntegrity,
    FormalStatementIntegrity,
    FormalStatementMismatch,
    BackendMismatch,
    InvocationMismatch,
    SourceDigestMismatch,
    ObligationIntegrity,
    ObligationFormalMismatch,
    LockIntegrity,
    Drift(DriftReport),
    JobConstruction(String),
    Evidence(EvidenceError),
    ProofArtifact(ProofArtifactError),
}

impl fmt::Display for VerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Lean verification I/O error: {error}"),
            Self::VerificationJobIntegrity => {
                write!(formatter, "content-addressed verification job id mismatch")
            }
            Self::FormalStatementIntegrity => write!(formatter, "formal statement id mismatch"),
            Self::FormalStatementMismatch => write!(
                formatter,
                "verification job formal statement does not match supplied formal statement"
            ),
            Self::BackendMismatch => {
                write!(formatter, "verification job is not for the Lean backend")
            }
            Self::InvocationMismatch => write!(
                formatter,
                "verification job invocation does not match the pinned Lean kernel contract"
            ),
            Self::SourceDigestMismatch => write!(
                formatter,
                "Lean source digest does not match the formal statement and verification job"
            ),
            Self::ObligationIntegrity => write!(formatter, "proof obligation id mismatch"),
            Self::ObligationFormalMismatch => write!(
                formatter,
                "proof obligation formal statement does not match supplied formal statement"
            ),
            Self::LockIntegrity => write!(formatter, "environment lock id mismatch"),
            Self::Drift(report) => write!(formatter, "verification refused due to {report}"),
            Self::JobConstruction(detail) => {
                write!(formatter, "verification job construction failed: {detail}")
            }
            Self::Evidence(error) => {
                write!(formatter, "typed evidence kernel result failed: {error}")
            }
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
            Self::Evidence(error) => Some(error),
            Self::VerificationJobIntegrity
            | Self::FormalStatementIntegrity
            | Self::FormalStatementMismatch
            | Self::BackendMismatch
            | Self::InvocationMismatch
            | Self::SourceDigestMismatch
            | Self::ObligationIntegrity
            | Self::ObligationFormalMismatch
            | Self::LockIntegrity
            | Self::Drift(_)
            | Self::JobConstruction(_) => None,
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

impl From<EvidenceError> for VerificationError {
    fn from(error: EvidenceError) -> Self {
        Self::Evidence(error)
    }
}

/// Fail-closed reproduce error at the Lean boundary.
#[derive(Debug)]
pub enum LeanReproduceError {
    Core(CoreReproduceError),
    Verification(VerificationError),
    KernelRejected,
}

impl fmt::Display for LeanReproduceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => write!(formatter, "{error}"),
            Self::Verification(error) => write!(formatter, "{error}"),
            Self::KernelRejected => {
                write!(
                    formatter,
                    "Lean kernel rejected the proof during reproduction"
                )
            }
        }
    }
}

impl std::error::Error for LeanReproduceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Core(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::KernelRejected => None,
        }
    }
}

impl From<CoreReproduceError> for LeanReproduceError {
    fn from(value: CoreReproduceError) -> Self {
        Self::Core(value)
    }
}

impl From<VerificationError> for LeanReproduceError {
    fn from(value: VerificationError) -> Self {
        Self::Verification(value)
    }
}

/// Classify a raw Lean process result into a typed PL-2.0 [`KernelOutcome`].
///
/// Mapping (fail-closed, non-upgradable):
/// - ordinary process success with exit code `0` → [`KernelOutcome::Accepted`]
/// - ordinary process failure with an exit code → [`KernelOutcome::Rejected`]
/// - an elapsed wall-clock budget → [`KernelOutcome::Timeout`] with measured time
/// - a signal or inconsistent process record → [`KernelOutcome::Unknown`]
#[must_use]
pub fn kernel_outcome_from_process(
    process: &LeanProcessResult,
    backend: FormalBackend,
    invocation: impl Into<String>,
) -> KernelOutcome {
    let invocation = invocation.into();
    if process.termination == ProcessTermination::Exited
        && process.accepted
        && process.exit_code == Some(0)
    {
        KernelOutcome::Accepted {
            receipt: KernelReceipt::new(
                backend,
                invocation,
                true,
                process.exit_code,
                process.stdout_digest,
                process.stderr_digest,
            ),
        }
    } else if let ProcessTermination::TimedOut { elapsed_ms } = process.termination {
        KernelOutcome::Timeout { elapsed_ms }
    } else if let ProcessTermination::Signaled { signal } = process.termination {
        KernelOutcome::Unknown {
            reason: format!("Lean process terminated by signal {signal:?}"),
        }
    } else if !process.accepted {
        if let Some(code) = process.exit_code {
            KernelOutcome::Rejected {
                receipt: KernelReceipt::new(
                    backend,
                    invocation,
                    false,
                    Some(code),
                    process.stdout_digest,
                    process.stderr_digest,
                ),
            }
        } else {
            KernelOutcome::Unknown {
                reason: "Lean process exited without an exit code or signal record".into(),
            }
        }
    } else {
        KernelOutcome::Unknown {
            reason: format!(
                "unclassified lean process result: accepted={}, exit_code={:?}",
                process.accepted, process.exit_code
            ),
        }
    }
}

/// Configured Lean command boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeanKernel {
    lake_binary: PathBuf,
    limits: LeanProcessLimits,
    isolation: Option<BubblewrapIsolation>,
}

impl Default for LeanKernel {
    fn default() -> Self {
        Self {
            lake_binary: PathBuf::from("lake"),
            limits: LeanProcessLimits::default(),
            isolation: None,
        }
    }
}

impl LeanKernel {
    #[must_use]
    pub fn new(lake_binary: impl Into<PathBuf>) -> Self {
        Self {
            lake_binary: lake_binary.into(),
            limits: LeanProcessLimits::default(),
            isolation: None,
        }
    }

    /// Override the bounded process and output budgets for subsequent invocations.
    #[must_use]
    pub fn with_limits(mut self, limits: LeanProcessLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Configure the mandatory OS boundary used for generated or third-party sources.
    #[must_use]
    pub fn with_isolation(mut self, isolation: BubblewrapIsolation) -> Self {
        self.isolation = Some(isolation);
        self
    }

    #[must_use]
    pub const fn limits(&self) -> LeanProcessLimits {
        self.limits
    }

    /// Stable invocation contract bound into verification jobs and receipts.
    #[must_use]
    pub fn invocation_contract(&self) -> String {
        self.limits.invocation_contract()
    }

    /// Stable invocation contract for untrusted-source verification.
    ///
    /// # Errors
    ///
    /// Fails closed when no OS isolation backend has been configured.
    pub fn isolated_invocation_contract(&self) -> std::io::Result<String> {
        let isolation = self.isolation.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "untrusted Lean verification requires an OS isolation backend",
            )
        })?;
        Ok(format!(
            "{};{}",
            self.limits.invocation_contract(),
            isolation.invocation_contract()
        ))
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
    /// represented by `LeanProcessResult::accepted == false` and is not an I/O error.
    pub fn verify_file(&self, source: impl AsRef<Path>) -> std::io::Result<LeanProcessResult> {
        let source = source.as_ref().canonicalize()?;
        let project_root = lake_project_root(&source)?;
        let mut command = Command::new(&self.lake_binary);
        command
            .current_dir(project_root)
            .arg("env")
            .arg("lean")
            .arg(&source);
        self.run_command(&mut command)
    }

    /// Verify generated or third-party Lean only through the configured OS boundary.
    ///
    /// The project and runtime trees are mounted read-only, network access is
    /// denied, and kernel resource limits are applied. This method fails closed
    /// when no isolation backend is configured; the supervised [`Self::verify_file`]
    /// path is intentionally not an acceptable fallback.
    ///
    /// # Errors
    ///
    /// Returns an I/O error for a missing/misconfigured isolation backend, an
    /// invalid source location, process launch failure or bounded-output failure.
    pub fn verify_untrusted_file(
        &self,
        source: impl AsRef<Path>,
    ) -> std::io::Result<LeanProcessResult> {
        let source = source.as_ref().canonicalize()?;
        let project_root = lake_project_root(&source)?;
        let isolation = self.isolation.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "untrusted Lean verification requires an OS isolation backend",
            )
        })?;
        let mut command = isolation.command(&self.lake_binary, &project_root, &source)?;
        self.run_command(&mut command)
    }

    fn run_command(&self, command: &mut Command) -> std::io::Result<LeanProcessResult> {
        let output = process_guard::run_command(command, self.limits)?;

        Ok(LeanProcessResult {
            accepted: output.status.success() && output.termination == ProcessTermination::Exited,
            exit_code: output.status.code(),
            termination: output.termination,
            limits: self.limits,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            stdout_digest: sha256_bytes(&output.stdout),
            stderr_digest: sha256_bytes(&output.stderr),
        })
    }

    /// Verify a content-addressed core job against an exact formal statement and source file.
    ///
    /// The core job is the authoritative request identity. The filesystem path is
    /// execution plumbing only: its bytes must match both the formal statement and
    /// the job's proof-source digest before Lean is invoked. A rejected Lean process
    /// returns `Ok(VerificationOutcome { proof: None, .. })`; it never becomes a
    /// proof artifact.
    ///
    /// # Errors
    ///
    /// Fails before kernel invocation when the job identity, formal statement,
    /// backend, invocation, or source binding is inconsistent. I/O failures and
    /// proof-artifact invariant failures are surfaced explicitly.
    pub fn verify_job(
        &self,
        job: &VerificationJob,
        formal_statement: &FormalStatement,
        source: impl AsRef<Path>,
    ) -> Result<VerificationOutcome, VerificationError> {
        self.verify_job_inner(job, formal_statement, source.as_ref(), None, false)
    }

    /// Verify a content-addressed job whose source is generated or third-party.
    ///
    /// # Errors
    ///
    /// Applies the same integrity checks as [`Self::verify_job`] and additionally
    /// fails closed unless the job identity and execution use the configured OS
    /// isolation boundary.
    pub fn verify_untrusted_job(
        &self,
        job: &VerificationJob,
        formal_statement: &FormalStatement,
        source: impl AsRef<Path>,
    ) -> Result<VerificationOutcome, VerificationError> {
        self.verify_job_inner(job, formal_statement, source.as_ref(), None, true)
    }

    /// Like [`Self::verify_job`], but fail closed unless `observed` binds the job's
    /// pinned environment lock.
    ///
    /// # Errors
    ///
    /// Returns [`VerificationError::Drift`] or [`VerificationError::LockIntegrity`]
    /// before Lean runs when the observed environment does not bind the job lock.
    /// Otherwise the same failures as [`Self::verify_job`] apply.
    pub fn verify_job_with_lock(
        &self,
        job: &VerificationJob,
        formal_statement: &FormalStatement,
        source: impl AsRef<Path>,
        observed: &EnvironmentLock,
    ) -> Result<VerificationOutcome, VerificationError> {
        self.verify_job_inner(
            job,
            formal_statement,
            source.as_ref(),
            Some(observed),
            false,
        )
    }

    /// Verify an untrusted job while also binding its environment lock.
    ///
    /// # Errors
    ///
    /// Combines the fail-closed OS-isolation requirement of
    /// [`Self::verify_untrusted_job`] with the drift checks of
    /// [`Self::verify_job_with_lock`].
    pub fn verify_untrusted_job_with_lock(
        &self,
        job: &VerificationJob,
        formal_statement: &FormalStatement,
        source: impl AsRef<Path>,
        observed: &EnvironmentLock,
    ) -> Result<VerificationOutcome, VerificationError> {
        self.verify_job_inner(job, formal_statement, source.as_ref(), Some(observed), true)
    }

    /// Verify a PL-2.0 [`ProofObligation`] through Lean into a typed [`KernelResult`].
    ///
    /// Emits `Accepted` / `Rejected` / `Unknown` / `Timeout` via
    /// [`kernel_outcome_from_process`]. A [`ProofArtifact`] is sealed only when
    /// the typed result converts to [`prooflab_core::AcceptedKernel`]; rejected,
    /// unknown and timeout outcomes never produce an artifact.
    ///
    /// # Errors
    ///
    /// Fails closed on obligation / job / formal integrity problems, I/O errors,
    /// typed evidence construction failures, or proof-artifact invariant failures.
    pub fn verify_obligation(
        &self,
        obligation: &ProofObligation,
        formal_statement: &FormalStatement,
        job: &VerificationJob,
        source: impl AsRef<Path>,
    ) -> Result<EvidenceVerificationOutcome, VerificationError> {
        self.verify_obligation_inner(
            obligation,
            formal_statement,
            job,
            source.as_ref(),
            None,
            false,
        )
    }

    /// Verify an untrusted proof obligation through the configured OS boundary.
    ///
    /// # Errors
    ///
    /// Applies the same typed evidence and integrity checks as
    /// [`Self::verify_obligation`] and fails closed without OS isolation.
    pub fn verify_untrusted_obligation(
        &self,
        obligation: &ProofObligation,
        formal_statement: &FormalStatement,
        job: &VerificationJob,
        source: impl AsRef<Path>,
    ) -> Result<EvidenceVerificationOutcome, VerificationError> {
        self.verify_obligation_inner(
            obligation,
            formal_statement,
            job,
            source.as_ref(),
            None,
            true,
        )
    }

    /// Like [`Self::verify_obligation`], but fail closed unless `observed` binds
    /// the job's pinned environment lock.
    ///
    /// # Errors
    ///
    /// Returns [`VerificationError::Drift`] or [`VerificationError::LockIntegrity`]
    /// before Lean runs when the observed environment does not bind the job lock.
    /// Otherwise the same failures as [`Self::verify_obligation`] apply.
    pub fn verify_obligation_with_lock(
        &self,
        obligation: &ProofObligation,
        formal_statement: &FormalStatement,
        job: &VerificationJob,
        source: impl AsRef<Path>,
        observed: &EnvironmentLock,
    ) -> Result<EvidenceVerificationOutcome, VerificationError> {
        self.verify_obligation_inner(
            obligation,
            formal_statement,
            job,
            source.as_ref(),
            Some(observed),
            false,
        )
    }

    /// Verify an untrusted obligation while also binding its environment lock.
    ///
    /// # Errors
    ///
    /// Combines the fail-closed OS-isolation requirement of
    /// [`Self::verify_untrusted_obligation`] with the drift checks of
    /// [`Self::verify_obligation_with_lock`].
    pub fn verify_untrusted_obligation_with_lock(
        &self,
        obligation: &ProofObligation,
        formal_statement: &FormalStatement,
        job: &VerificationJob,
        source: impl AsRef<Path>,
        observed: &EnvironmentLock,
    ) -> Result<EvidenceVerificationOutcome, VerificationError> {
        self.verify_obligation_inner(
            obligation,
            formal_statement,
            job,
            source.as_ref(),
            Some(observed),
            true,
        )
    }

    /// Reproduce a stored proof artifact under an observed environment lock.
    ///
    /// This first runs the core integrity/drift/dependency checks, then
    /// re-invokes Lean under the artifact's locked `ReproMeta` via a fresh
    /// [`VerificationJob`]. Lean acceptance yields a new sealed proof artifact
    /// through [`ProofArtifact::new_verified`]; rejection fails closed.
    ///
    /// Reproduce success is an environment/re-verification result. It does not
    /// by itself transition any claim to `PROVED`.
    ///
    /// # Errors
    ///
    /// Fails closed on integrity, drift, missing dependencies, verification
    /// contract violations, I/O errors, or Lean rejection.
    pub fn reproduce(
        &self,
        artifact: &ProofArtifact,
        observed: &EnvironmentLock,
        formal_statement: &FormalStatement,
        source: impl AsRef<Path>,
        available_dependencies: Option<&BTreeSet<ProofArtifactId>>,
    ) -> Result<ReproduceOutcome, LeanReproduceError> {
        let environment = core_reproduce(artifact, observed, available_dependencies)?;

        let source = source.as_ref();
        let source_bytes = fs::read(source).map_err(VerificationError::from)?;
        if !formal_statement.matches_source(&source_bytes)
            || artifact.body.proof_source_digest != sha256_bytes(&source_bytes)
        {
            return Err(VerificationError::SourceDigestMismatch.into());
        }
        if artifact.body.formal_statement_id != formal_statement.id {
            return Err(VerificationError::FormalStatementMismatch.into());
        }

        let job = VerificationJob::new_with_dependencies(
            formal_statement,
            &source_bytes,
            artifact.body.dependencies.clone(),
            self.invocation_contract(),
            artifact.body.repro.clone(),
        )
        .map_err(|error| VerificationError::JobConstruction(error.to_string()))?;

        let outcome = self.verify_job_with_lock(&job, formal_statement, source, observed)?;

        if !outcome.result.accepted {
            return Err(LeanReproduceError::KernelRejected);
        }
        let Some(reverified) = outcome.proof else {
            return Err(LeanReproduceError::KernelRejected);
        };

        Ok(ReproduceOutcome {
            environment,
            result: outcome.result,
            reverified: Some(reverified),
        })
    }

    fn verify_job_inner(
        &self,
        job: &VerificationJob,
        formal_statement: &FormalStatement,
        source: &Path,
        observed: Option<&EnvironmentLock>,
        untrusted: bool,
    ) -> Result<VerificationOutcome, VerificationError> {
        let (result, source_bytes) =
            self.run_verified_process(job, formal_statement, source, observed, untrusted)?;
        let proof = if result.accepted {
            let receipt = KernelReceipt::new(
                job.backend,
                job.invocation.clone(),
                true,
                result.exit_code,
                result.stdout_digest,
                result.stderr_digest,
            );
            Some(ProofArtifact::new_verified(
                formal_statement,
                &source_bytes,
                job.dependencies.clone(),
                job.repro.clone(),
                receipt,
            )?)
        } else {
            None
        };
        Ok(VerificationOutcome { result, proof })
    }

    fn verify_obligation_inner(
        &self,
        obligation: &ProofObligation,
        formal_statement: &FormalStatement,
        job: &VerificationJob,
        source: &Path,
        observed: Option<&EnvironmentLock>,
        untrusted: bool,
    ) -> Result<EvidenceVerificationOutcome, VerificationError> {
        if !obligation.check_id() {
            return Err(VerificationError::ObligationIntegrity);
        }
        if obligation.formal_statement_id != formal_statement.id {
            return Err(VerificationError::ObligationFormalMismatch);
        }

        let (process, source_bytes) =
            self.run_verified_process(job, formal_statement, source, observed, untrusted)?;
        let kernel_outcome =
            kernel_outcome_from_process(&process, job.backend, job.invocation.clone());
        let kernel_result = KernelResult::new(obligation, formal_statement, kernel_outcome)?;
        let proof = match kernel_result.clone().into_accepted() {
            Ok(accepted) => Some(accepted.seal_proof_artifact(
                formal_statement,
                &source_bytes,
                job.dependencies.clone(),
                job.repro.clone(),
            )?),
            Err(_) => None,
        };

        Ok(EvidenceVerificationOutcome {
            process,
            kernel_result,
            proof,
        })
    }

    /// Shared prechecks + Lean process invocation for job and obligation paths.
    fn run_verified_process(
        &self,
        job: &VerificationJob,
        formal_statement: &FormalStatement,
        source: &Path,
        observed: Option<&EnvironmentLock>,
        untrusted: bool,
    ) -> Result<(LeanProcessResult, Vec<u8>), VerificationError> {
        if !job.check_id() {
            return Err(VerificationError::VerificationJobIntegrity);
        }
        if !formal_statement.check_id() {
            return Err(VerificationError::FormalStatementIntegrity);
        }
        if job.formal_statement_id != formal_statement.id {
            return Err(VerificationError::FormalStatementMismatch);
        }
        if job.backend != FormalBackend::Lean4 || formal_statement.backend != FormalBackend::Lean4 {
            return Err(VerificationError::BackendMismatch);
        }
        let invocation = if untrusted {
            self.isolated_invocation_contract()?
        } else {
            self.invocation_contract()
        };
        if job.invocation != invocation {
            return Err(VerificationError::InvocationMismatch);
        }

        if let Some(observed) = observed {
            if !observed.check_id() {
                return Err(VerificationError::LockIntegrity);
            }
            let expected = EnvironmentLock::from_repro(&job.repro)
                .map_err(|error| VerificationError::JobConstruction(error.to_string()))?;
            let drift = expected.compare(observed);
            if !drift.binds() {
                return Err(VerificationError::Drift(drift));
            }
        }

        let source_bytes = fs::read(source)?;
        if !formal_statement.matches_source(&source_bytes)
            || !job.matches_proof_source(&source_bytes)
        {
            return Err(VerificationError::SourceDigestMismatch);
        }

        let result = if untrusted {
            self.verify_untrusted_file(source)?
        } else {
            self.verify_file(source)?
        };
        Ok((result, source_bytes))
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
    use prooflab_core::{
        Claim, ClaimBody, ClaimStatus, ConjectureCandidate, EvidenceClaim, EvidenceStrength,
        FormalStatement, FormalizationAuthority, FormalizationMeta, Observation,
        PromotionAuthority, PromotionMeta, ProofObligation, ReproMeta, sha256_bytes,
    };

    use super::*;

    fn repro() -> ReproMeta {
        let mut repro = ReproMeta::bootstrap("test-environment");
        repro.prooflab_revision = "test-revision".into();
        repro
    }

    #[test]
    fn default_boundary_uses_lake() {
        assert_eq!(LeanKernel::default().lake_binary, PathBuf::from("lake"));
        assert_eq!(
            LeanKernel::default().invocation_contract(),
            DEFAULT_LEAN_INVOCATION
        );
    }

    #[test]
    fn supervisor_limits_are_part_of_the_job_identity() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ProofLab/Core/Smoke.lean");
        let source_bytes = fs::read(&source).unwrap();
        let claim = Claim::new(ClaimBody {
            statement: "forall n : Nat, n = n".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let formal = FormalStatement::lean4(claim.id, &source_bytes, vec!["Mathlib".into()]);
        let job = VerificationJob::new(
            &formal,
            &source_bytes,
            LeanKernel::default().invocation_contract(),
            repro(),
        )
        .unwrap();
        let kernel = LeanKernel::new("this-command-must-not-run").with_limits(LeanProcessLimits {
            timeout: Duration::from_secs(1),
            ..LeanProcessLimits::default()
        });
        assert_ne!(job.invocation, kernel.invocation_contract());
        assert!(matches!(
            kernel.verify_job(&job, &formal, source),
            Err(VerificationError::InvocationMismatch)
        ));
    }

    #[test]
    fn untrusted_source_fails_closed_without_isolation() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ProofLab/Core/Smoke.lean");
        let error = LeanKernel::default()
            .verify_untrusted_file(source)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            error
                .to_string()
                .contains("requires an OS isolation backend")
        );
    }

    #[test]
    fn isolated_jobs_have_a_distinct_content_addressed_contract() {
        let kernel = LeanKernel::new("/usr/bin/false").with_isolation(BubblewrapIsolation::new(
            "/usr/bin/bwrap",
            "/usr/bin/prlimit",
        ));
        let isolated = kernel.isolated_invocation_contract().unwrap();
        assert_ne!(isolated, kernel.invocation_contract());
        assert!(isolated.contains("isolation=linux-setuid-bubblewrap-v1"));
        assert!(isolated.contains("network=deny_all"));
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
        let job = VerificationJob::new(
            &formal,
            b"different source",
            DEFAULT_LEAN_INVOCATION,
            repro(),
        )
        .unwrap();
        let kernel = LeanKernel::new("this-command-must-not-run");
        assert!(matches!(
            kernel.verify_job(&job, &formal, source),
            Err(VerificationError::SourceDigestMismatch)
        ));
    }

    #[test]
    fn tampered_core_job_fails_before_kernel_invocation() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ProofLab/Core/Smoke.lean");
        let source_bytes = fs::read(&source).unwrap();
        let claim = Claim::new(ClaimBody {
            statement: "forall n : Nat, n = n".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let formal = FormalStatement::lean4(claim.id, &source_bytes, vec!["Mathlib".into()]);
        let mut job =
            VerificationJob::new(&formal, &source_bytes, DEFAULT_LEAN_INVOCATION, repro()).unwrap();
        job.invocation = "lean directly".into();
        let kernel = LeanKernel::new("this-command-must-not-run");
        assert!(matches!(
            kernel.verify_job(&job, &formal, source),
            Err(VerificationError::VerificationJobIntegrity)
        ));
    }

    #[test]
    fn drifted_lock_fails_before_kernel_invocation() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ProofLab/Core/Smoke.lean");
        let source_bytes = fs::read(&source).unwrap();
        let claim = Claim::new(ClaimBody {
            statement: "forall n : Nat, n = n".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let formal = FormalStatement::lean4(claim.id, &source_bytes, vec!["Mathlib".into()]);
        let job =
            VerificationJob::new(&formal, &source_bytes, DEFAULT_LEAN_INVOCATION, repro()).unwrap();
        let observed = EnvironmentLock::new(
            "v4.33.1",
            "0df444a360eaa60ab8c11dca51a86af692955474",
            "test-revision",
            "different-environment",
        )
        .unwrap();
        let kernel = LeanKernel::new("this-command-must-not-run");
        assert!(matches!(
            kernel.verify_job_with_lock(&job, &formal, source, &observed),
            Err(VerificationError::Drift(_))
        ));
    }

    fn process(
        accepted: bool,
        exit_code: Option<i32>,
        termination: ProcessTermination,
        stdout: &str,
        stderr: &str,
    ) -> LeanProcessResult {
        LeanProcessResult {
            accepted,
            exit_code,
            termination,
            limits: LeanProcessLimits::default(),
            stdout: stdout.into(),
            stderr: stderr.into(),
            stdout_digest: sha256_bytes(stdout.as_bytes()),
            stderr_digest: sha256_bytes(stderr.as_bytes()),
        }
    }

    fn obligation_pipeline(formal_source: &[u8]) -> (FormalStatement, ProofObligation) {
        let claim = Claim::new(ClaimBody {
            statement: "n = n".into(),
            assumptions: vec!["n : Nat".into()],
            parents: vec![],
        });
        let observation = Observation::stub("stub://pl-2.0/lean-bridge", b"samples");
        let evidence = EvidenceClaim::from_observations(
            claim.id,
            &[&observation],
            EvidenceStrength::Suggestive,
        )
        .unwrap();
        let promotion = PromotionMeta::new(
            PromotionAuthority::Human,
            "lean-bridge-test",
            "stage-1 explicit promotion for obligation pipeline",
        )
        .unwrap();
        let conjecture = ConjectureCandidate::from_evidence(
            claim.id,
            &[&evidence],
            "n = n",
            vec!["n : Nat".into()],
            promotion,
        )
        .unwrap();
        let formal = FormalStatement::lean4(claim.id, formal_source, vec!["Mathlib".into()]);
        let formalization = FormalizationMeta::new(
            FormalizationAuthority::Agent,
            "lean-bridge-formalizer",
            "bind the promoted conjecture to this Lean formal statement",
        )
        .unwrap();
        let obligation =
            ProofObligation::from_conjecture(&conjecture, &formal, formalization).unwrap();
        assert_eq!(obligation.status, ClaimStatus::Formalized);
        (formal, obligation)
    }

    #[test]
    fn process_classification_emits_distinct_kernel_outcomes() {
        let accepted = kernel_outcome_from_process(
            &process(true, Some(0), ProcessTermination::Exited, "ok", ""),
            FormalBackend::Lean4,
            DEFAULT_LEAN_INVOCATION,
        );
        assert!(matches!(accepted, KernelOutcome::Accepted { .. }));
        assert!(accepted.is_accepting());

        let rejected = kernel_outcome_from_process(
            &process(false, Some(1), ProcessTermination::Exited, "", "error"),
            FormalBackend::Lean4,
            DEFAULT_LEAN_INVOCATION,
        );
        assert!(matches!(rejected, KernelOutcome::Rejected { .. }));
        assert!(!rejected.is_accepting());

        let timeout = kernel_outcome_from_process(
            &process(
                false,
                None,
                ProcessTermination::TimedOut { elapsed_ms: 37 },
                "",
                "killed",
            ),
            FormalBackend::Lean4,
            DEFAULT_LEAN_INVOCATION,
        );
        assert!(matches!(timeout, KernelOutcome::Timeout { elapsed_ms: 37 }));
        assert!(!timeout.is_accepting());

        let unknown = kernel_outcome_from_process(
            &process(true, Some(2), ProcessTermination::Exited, "weird", ""),
            FormalBackend::Lean4,
            DEFAULT_LEAN_INVOCATION,
        );
        assert!(matches!(unknown, KernelOutcome::Unknown { .. }));
        assert!(!unknown.is_accepting());
    }

    #[test]
    fn non_accepting_outcomes_cannot_become_accepted_kernel() {
        let (formal, obligation) = obligation_pipeline(b"theorem t : True := trivial\n");
        for outcome in [
            kernel_outcome_from_process(
                &process(false, Some(1), ProcessTermination::Exited, "", "err"),
                FormalBackend::Lean4,
                DEFAULT_LEAN_INVOCATION,
            ),
            KernelOutcome::Unknown {
                reason: "garbled".into(),
            },
            KernelOutcome::Timeout { elapsed_ms: 5 },
        ] {
            let result = KernelResult::new(&obligation, &formal, outcome).unwrap();
            assert!(result.into_accepted().is_err());
        }
    }

    #[test]
    fn accepted_process_seals_only_via_accepted_kernel() {
        let source = b"theorem t : True := trivial\n";
        let (formal, obligation) = obligation_pipeline(source);
        let outcome = kernel_outcome_from_process(
            &process(true, Some(0), ProcessTermination::Exited, "ok", ""),
            FormalBackend::Lean4,
            DEFAULT_LEAN_INVOCATION,
        );
        let result = KernelResult::new(&obligation, &formal, outcome).unwrap();
        let accepted = result.into_accepted().expect("accepted");
        let repro = repro();
        let artifact = accepted
            .seal_proof_artifact(&formal, source, vec![], repro)
            .expect("sealed");
        assert!(artifact.check_id());
        assert_eq!(artifact.body.formal_statement_id, formal.id);
    }

    #[test]
    fn verify_obligation_fails_closed_on_obligation_mismatch_before_lean() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ProofLab/Core/Smoke.lean");
        let source_bytes = fs::read(&source).unwrap();
        let (formal, obligation) = obligation_pipeline(&source_bytes);
        let other_claim = Claim::new(ClaimBody {
            statement: "other".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let other_formal =
            FormalStatement::lean4(other_claim.id, &source_bytes, vec!["Mathlib".into()]);
        let job =
            VerificationJob::new(&formal, &source_bytes, DEFAULT_LEAN_INVOCATION, repro()).unwrap();
        let kernel = LeanKernel::new("this-command-must-not-run");
        assert!(matches!(
            kernel.verify_obligation(&obligation, &other_formal, &job, &source),
            Err(VerificationError::ObligationFormalMismatch)
        ));

        let mut tampered = obligation.clone();
        tampered.formal_statement_id.0[0] ^= 0xff;
        assert!(matches!(
            kernel.verify_obligation(&tampered, &formal, &job, &source),
            Err(VerificationError::ObligationIntegrity)
        ));
    }

    #[test]
    fn verify_obligation_source_mismatch_fails_before_lean() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ProofLab/Core/Smoke.lean");
        let (formal, obligation) = obligation_pipeline(b"different source");
        let job = VerificationJob::new(
            &formal,
            b"different source",
            DEFAULT_LEAN_INVOCATION,
            repro(),
        )
        .unwrap();
        let kernel = LeanKernel::new("this-command-must-not-run");
        assert!(matches!(
            kernel.verify_obligation(&obligation, &formal, &job, source),
            Err(VerificationError::SourceDigestMismatch)
        ));
    }

    #[cfg(unix)]
    mod process_supervision {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{Duration, Instant};

        use super::*;

        static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

        struct FakeLake {
            root: PathBuf,
            binary: PathBuf,
            source: PathBuf,
        }

        impl FakeLake {
            fn new(body: &str) -> Self {
                let unique = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
                let root = std::env::temp_dir()
                    .join(format!("prooflab-process-{}-{unique}", std::process::id()));
                fs::create_dir_all(&root).unwrap();
                fs::write(root.join("lakefile.lean"), "").unwrap();
                let source = root.join("Input.lean");
                fs::write(&source, "theorem input : True := trivial\n").unwrap();
                let binary = root.join("fake-lake");
                fs::write(&binary, format!("#!/bin/sh\n{body}\n")).unwrap();
                let mut permissions = fs::metadata(&binary).unwrap().permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&binary, permissions).unwrap();
                Self {
                    root,
                    binary,
                    source,
                }
            }

            fn kernel(&self) -> LeanKernel {
                self.kernel_with_timeout(Duration::from_millis(150))
            }

            fn kernel_with_timeout(&self, timeout: Duration) -> LeanKernel {
                LeanKernel::new(&self.binary).with_limits(LeanProcessLimits {
                    timeout,
                    drain_timeout: Duration::from_millis(250),
                    termination_grace: Duration::from_millis(250),
                    max_stdout_bytes: 128,
                    max_stderr_bytes: 128,
                })
            }
        }

        impl Drop for FakeLake {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.root);
            }
        }

        #[test]
        fn timeout_terminates_group_and_reports_measured_timeout() {
            let fixture = FakeLake::new("sleep 30 &\nwait");
            let started = Instant::now();
            let result = fixture.kernel().verify_file(&fixture.source).unwrap();
            assert!(started.elapsed() < Duration::from_secs(2));
            assert!(!result.accepted);
            assert!(matches!(
                result.termination,
                ProcessTermination::TimedOut { elapsed_ms } if elapsed_ms >= 100
            ));
            assert_eq!(result.limits, fixture.kernel().limits());
            assert!(matches!(
                kernel_outcome_from_process(&result, FormalBackend::Lean4, DEFAULT_LEAN_INVOCATION,),
                KernelOutcome::Timeout { elapsed_ms } if elapsed_ms >= 100
            ));
        }

        #[test]
        fn zero_exit_after_deadline_is_not_accepted() {
            let fixture = FakeLake::new("sleep 0.05\nexit 0");
            let result = fixture
                .kernel_with_timeout(Duration::from_millis(20))
                .verify_file(&fixture.source)
                .unwrap();
            assert!(!result.accepted);
            assert!(matches!(
                result.termination,
                ProcessTermination::TimedOut { .. }
            ));
        }

        #[test]
        fn parent_exit_cleans_descendant_that_retains_pipes() {
            let fixture = FakeLake::new("sleep 30 &\nexit 0");
            let started = Instant::now();
            let result = fixture.kernel().verify_file(&fixture.source).unwrap();
            assert!(started.elapsed() < Duration::from_secs(2));
            assert!(result.accepted);
            assert_eq!(result.termination, ProcessTermination::Exited);
        }

        #[test]
        fn signal_is_unknown_and_never_mislabeled_as_timeout() {
            let fixture = FakeLake::new("kill -TERM $$");
            let result = fixture.kernel().verify_file(&fixture.source).unwrap();
            assert!(!result.accepted);
            assert!(matches!(
                result.termination,
                ProcessTermination::Signaled { signal: Some(15) }
            ));
            assert!(matches!(
                kernel_outcome_from_process(&result, FormalBackend::Lean4, DEFAULT_LEAN_INVOCATION,),
                KernelOutcome::Unknown { .. }
            ));
        }

        #[test]
        fn output_over_limit_fails_closed_without_unbounded_retention() {
            let fixture = FakeLake::new(
                "printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'",
            );
            let error = fixture.kernel().verify_file(&fixture.source).unwrap_err();
            assert!(error.to_string().contains("output limit"));
        }
    }
}
