//! PL-1.0 known-theorem reproduction corpus.
//!
//! The corpus measures Lean verification and orchestration correctness only.
//! Entries are already-known facts or intentional falsehoods. Matching an
//! expected accept/reject outcome never authorizes scientific novelty claims,
//! and only [`prooflab_core::ProofArtifact::new_verified`] after kernel
//! acceptance seals proof status.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use prooflab_core::{
    Claim, ClaimBody, EnvironmentLock, FormalStatement, ReproMeta, VerificationJob,
};

use crate::{LeanKernel, VerificationError, VerificationOutcome};

const LEAN_INVOCATION: &str = "lake env lean";

/// Expected trusted-kernel outcome for a corpus entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedOutcome {
    /// Lean must accept and a proof artifact must be sealed.
    Accept,
    /// Lean must reject and no proof artifact may be produced.
    Reject,
}

/// One already-known theorem or intentional rejection control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusEntry {
    pub id: &'static str,
    pub statement: &'static str,
    pub assumptions: &'static [&'static str],
    /// Path relative to the `ProofLab` repository root.
    pub relative_path: &'static str,
    pub imports: &'static [&'static str],
    pub proof_shape: &'static str,
    pub expected: ExpectedOutcome,
}

/// Built claim / formal statement / verification job for one corpus entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusPrepared {
    pub entry_id: &'static str,
    pub claim: Claim,
    pub formal_statement: FormalStatement,
    pub job: VerificationJob,
    pub source_path: PathBuf,
    pub source_bytes: Vec<u8>,
    pub expected: ExpectedOutcome,
}

/// Measured verification result for one corpus entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusRunReport {
    pub entry_id: &'static str,
    pub expected: ExpectedOutcome,
    pub accepted: bool,
    pub produced_proof: bool,
    pub matches_expectation: bool,
}

/// Fail-closed corpus orchestration error.
#[derive(Debug)]
pub enum CorpusError {
    Io(std::io::Error),
    MissingSource(PathBuf),
    JobConstruction(String),
    Verification(VerificationError),
}

impl fmt::Display for CorpusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "corpus I/O error: {error}"),
            Self::MissingSource(path) => {
                write!(formatter, "corpus source missing: {}", path.display())
            }
            Self::JobConstruction(detail) => {
                write!(
                    formatter,
                    "corpus verification job construction failed: {detail}"
                )
            }
            Self::Verification(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for CorpusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::MissingSource(_) | Self::JobConstruction(_) => None,
        }
    }
}

impl From<std::io::Error> for CorpusError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<VerificationError> for CorpusError {
    fn from(error: VerificationError) -> Self {
        Self::Verification(error)
    }
}

/// Minimal known-theorem corpus covering distinct proof shapes plus one reject fixture.
pub const KNOWN_THEOREM_CORPUS: &[CorpusEntry] = &[
    CorpusEntry {
        id: "nat-identity-rfl",
        statement: "forall n : Nat, n = n",
        assumptions: &["n : Nat"],
        relative_path: "ProofLab/Corpus/NatIdentity.lean",
        imports: &["Mathlib.Data.Nat.Basic"],
        proof_shape: "rfl",
        expected: ExpectedOutcome::Accept,
    },
    CorpusEntry {
        id: "nat-two-plus-two-decide",
        statement: "(2 : Nat) + 2 = 4",
        assumptions: &[],
        relative_path: "ProofLab/Corpus/NatDecide.lean",
        imports: &["Mathlib.Data.Nat.Basic"],
        proof_shape: "decide",
        expected: ExpectedOutcome::Accept,
    },
    CorpusEntry {
        id: "nat-add-zero-induction",
        statement: "forall n : Nat, 0 + n = n",
        assumptions: &["n : Nat"],
        relative_path: "ProofLab/Corpus/NatAddZero.lean",
        imports: &["Mathlib.Data.Nat.Basic"],
        proof_shape: "induction",
        expected: ExpectedOutcome::Accept,
    },
    CorpusEntry {
        id: "nat-zero-or-succ-cases",
        statement: "forall n : Nat, n = 0 ∨ ∃ m, n = Nat.succ m",
        assumptions: &["n : Nat"],
        relative_path: "ProofLab/Corpus/NatCases.lean",
        imports: &["Mathlib.Data.Nat.Basic"],
        proof_shape: "cases",
        expected: ExpectedOutcome::Accept,
    },
    CorpusEntry {
        id: "nat-false-decide-reject",
        statement: "(2 : Nat) + 2 = 5",
        assumptions: &[],
        relative_path: "ProofLab/Corpus/Fixtures/RejectedFalse.lean",
        imports: &["Mathlib.Data.Nat.Basic"],
        proof_shape: "decide-false",
        expected: ExpectedOutcome::Reject,
    },
];

impl CorpusEntry {
    /// Build the immutable claim for this corpus entry.
    #[must_use]
    pub fn claim(&self) -> Claim {
        Claim::new(ClaimBody {
            statement: self.statement.into(),
            assumptions: self
                .assumptions
                .iter()
                .map(|assumption| (*assumption).to_owned())
                .collect(),
            parents: vec![],
        })
    }

    /// Resolve the Lean source path under `repo_root`.
    #[must_use]
    pub fn source_path(&self, repo_root: impl AsRef<Path>) -> PathBuf {
        repo_root.as_ref().join(self.relative_path)
    }

    /// Load source bytes and construct the content-addressed formal statement + job.
    ///
    /// # Errors
    ///
    /// Returns [`CorpusError::MissingSource`] when the fixture file is absent,
    /// I/O errors when the file cannot be read, or job-construction failures when
    /// reproducibility metadata is incomplete.
    pub fn prepare(
        &self,
        repo_root: impl AsRef<Path>,
        repro: ReproMeta,
    ) -> Result<CorpusPrepared, CorpusError> {
        let source_path = self.source_path(repo_root);
        if !source_path.is_file() {
            return Err(CorpusError::MissingSource(source_path));
        }
        let source_bytes = fs::read(&source_path)?;
        let claim = self.claim();
        let formal_statement = FormalStatement::lean4(
            claim.id,
            &source_bytes,
            self.imports
                .iter()
                .map(|import| (*import).to_owned())
                .collect(),
        );
        let job = VerificationJob::new(&formal_statement, &source_bytes, LEAN_INVOCATION, repro)
            .map_err(|error| CorpusError::JobConstruction(error.to_string()))?;
        Ok(CorpusPrepared {
            entry_id: self.id,
            claim,
            formal_statement,
            job,
            source_path,
            source_bytes,
            expected: self.expected,
        })
    }
}

impl CorpusPrepared {
    /// Run the prepared job through the Lean trust boundary.
    ///
    /// When `observed` is provided, verification fails closed on environment drift
    /// before Lean is invoked.
    ///
    /// # Errors
    ///
    /// Propagates [`VerificationError`] from the Lean boundary. A Lean rejection
    /// itself is not an error when the entry expects rejection; it becomes a
    /// successful report with `accepted == false`.
    pub fn verify(
        &self,
        kernel: &LeanKernel,
        observed: Option<&EnvironmentLock>,
    ) -> Result<(VerificationOutcome, CorpusRunReport), CorpusError> {
        let outcome = match observed {
            Some(lock) => kernel.verify_job_with_lock(
                &self.job,
                &self.formal_statement,
                &self.source_path,
                lock,
            )?,
            None => kernel.verify_job(&self.job, &self.formal_statement, &self.source_path)?,
        };
        let produced_proof = outcome.proof.is_some();
        let matches_expectation = match self.expected {
            ExpectedOutcome::Accept => outcome.result.accepted && produced_proof,
            ExpectedOutcome::Reject => !outcome.result.accepted && !produced_proof,
        };
        let report = CorpusRunReport {
            entry_id: self.entry_id,
            expected: self.expected,
            accepted: outcome.result.accepted,
            produced_proof,
            matches_expectation,
        };
        Ok((outcome, report))
    }
}

/// Prepare every corpus entry under `repo_root`.
///
/// # Errors
///
/// Fails closed on the first entry that cannot be prepared.
pub fn prepare_corpus(
    repo_root: impl AsRef<Path>,
    repro: &ReproMeta,
) -> Result<Vec<CorpusPrepared>, CorpusError> {
    let repo_root = repo_root.as_ref();
    KNOWN_THEOREM_CORPUS
        .iter()
        .map(|entry| entry.prepare(repo_root, repro.clone()))
        .collect()
}

/// Verify the full known-theorem corpus and return per-entry reports.
///
/// # Errors
///
/// Fails closed on preparation or trust-boundary contract violations. Individual
/// Lean accept/reject outcomes are captured in the reports rather than turned
/// into orchestration errors.
pub fn verify_corpus(
    kernel: &LeanKernel,
    repo_root: impl AsRef<Path>,
    repro: &ReproMeta,
    observed: Option<&EnvironmentLock>,
) -> Result<Vec<(VerificationOutcome, CorpusRunReport)>, CorpusError> {
    prepare_corpus(repo_root, repro)?
        .into_iter()
        .map(|prepared| prepared.verify(kernel, observed))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prooflab_core::EnvironmentLock;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn repro() -> ReproMeta {
        let mut repro = ReproMeta::bootstrap("pl-1.0-known-theorem-corpus");
        repro.prooflab_revision = "corpus-unit-test".into();
        repro
    }

    #[test]
    fn corpus_covers_accept_and_reject_with_distinct_shapes() {
        let accepts = KNOWN_THEOREM_CORPUS
            .iter()
            .filter(|entry| entry.expected == ExpectedOutcome::Accept)
            .count();
        let rejects = KNOWN_THEOREM_CORPUS
            .iter()
            .filter(|entry| entry.expected == ExpectedOutcome::Reject)
            .count();
        assert!(
            accepts >= 3,
            "PL-1.0 requires several distinct accept shapes"
        );
        assert_eq!(
            rejects, 1,
            "PL-1.0 requires an intentional rejection fixture"
        );

        let mut shapes = KNOWN_THEOREM_CORPUS
            .iter()
            .filter(|entry| entry.expected == ExpectedOutcome::Accept)
            .map(|entry| entry.proof_shape)
            .collect::<Vec<_>>();
        shapes.sort_unstable();
        shapes.dedup();
        assert!(
            shapes.len() >= 3,
            "accept fixtures must cover distinct proof shapes"
        );
    }

    #[test]
    fn prepared_entries_bind_on_disk_sources() {
        for entry in KNOWN_THEOREM_CORPUS {
            let prepared = entry.prepare(repo_root(), repro()).expect("prepare");
            assert!(prepared.formal_statement.check_id());
            assert!(
                prepared
                    .formal_statement
                    .matches_source(&prepared.source_bytes)
            );
            assert!(prepared.job.check_id());
            assert!(prepared.job.matches_proof_source(&prepared.source_bytes));
            assert_eq!(
                prepared.job.formal_statement_id,
                prepared.formal_statement.id
            );
            assert_eq!(prepared.expected, entry.expected);
            assert!(
                prepared.source_path.is_file(),
                "missing {}",
                prepared.source_path.display()
            );
        }
    }

    #[test]
    fn drifted_lock_fails_before_kernel_for_corpus_entry() {
        let prepared = KNOWN_THEOREM_CORPUS[0]
            .prepare(repo_root(), repro())
            .expect("prepare");
        let observed = EnvironmentLock::new(
            "v4.33.1",
            "0df444a360eaa60ab8c11dca51a86af692955474",
            "corpus-unit-test",
            "different-environment",
        )
        .unwrap();
        let kernel = LeanKernel::new("this-command-must-not-run");
        assert!(matches!(
            prepared.verify(&kernel, Some(&observed)),
            Err(CorpusError::Verification(VerificationError::Drift(_)))
        ));
    }

    #[test]
    fn claim_identity_is_stable_for_corpus_statements() {
        let first = KNOWN_THEOREM_CORPUS[0].claim();
        let second = KNOWN_THEOREM_CORPUS[0].claim();
        assert_eq!(first.id, second.id);
        assert_ne!(first.id, KNOWN_THEOREM_CORPUS[1].claim().id);
    }
}
