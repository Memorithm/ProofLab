//! PL-1.2 assumption minimization.
//!
//! For known proved statements with redundant assumptions, automatically form
//! leave-one-out removal candidates and re-verify each candidate through the
//! Lean trust boundary.
//!
//! Outcomes:
//! - [`RemovalOutcome::Removable`] — Lean accepted after removal;
//! - [`RemovalOutcome::RemovalRejected`] — Lean rejected after removal.
//!
//! Kernel rejection alone never authorizes a necessity claim. Callers that wish
//! to argue an assumption was load-bearing must supply an independent
//! counterexample or mathematical argument for the weakened claim. Lean remains
//! the sole authority that may seal [`prooflab_core::ClaimStatus::Proved`].

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use prooflab_core::{
    Claim, ClaimBody, EnvironmentLock, FormalStatement, ReproMeta, VerificationJob,
};

use crate::{LeanKernel, VerificationError, VerificationOutcome};

const LEAN_INVOCATION: &str = "lake env lean";

/// Expected control outcome for one curated removal trial.
///
/// This is an orchestration expectation for the fixed PL-1.2 control set. It is
/// not a mathematical necessity judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedRemovalOutcome {
    /// Control expects Lean to accept the weakened statement.
    KernelAccepts,
    /// Control expects Lean to reject the weakened statement.
    ///
    /// Rejection measures re-verification failure only. It must not be reported
    /// as proof that the removed assumption was necessary.
    KernelRejects,
}

/// Measured kernel outcome for one automatic assumption-removal candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalOutcome {
    /// Lean accepted the statement after the assumption was removed.
    Removable,
    /// Lean rejected the statement after the assumption was removed.
    ///
    /// This is **not** a necessity certificate.
    RemovalRejected,
}

impl RemovalOutcome {
    /// Return whether this outcome matches a curated control expectation.
    #[must_use]
    pub fn matches_expectation(self, expected: ExpectedRemovalOutcome) -> bool {
        matches!(
            (self, expected),
            (Self::Removable, ExpectedRemovalOutcome::KernelAccepts)
                | (Self::RemovalRejected, ExpectedRemovalOutcome::KernelRejects)
        )
    }
}

/// One curated single-assumption removal paired with a Lean fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovalTrial {
    /// Index into [`MinimizationEntry::assumptions`] to drop.
    pub remove_index: usize,
    /// Human-readable label of the dropped assumption (for reports).
    pub removed_assumption: &'static str,
    /// Claim statement text after removal.
    pub weakened_statement: &'static str,
    /// Path relative to the repository root for the weakened Lean source.
    pub relative_path: &'static str,
    pub expected: ExpectedRemovalOutcome,
}

/// One known proved statement used as an assumption-minimization control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimizationEntry {
    pub id: &'static str,
    pub statement: &'static str,
    pub assumptions: &'static [&'static str],
    /// Path relative to the repository root for the full (pre-minimization) source.
    pub relative_path_full: &'static str,
    pub imports: &'static [&'static str],
    pub removal_trials: &'static [RemovalTrial],
}

/// Leave-one-out candidate produced by automatic assumption removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalCandidate {
    pub remove_index: usize,
    pub removed_assumption: String,
    pub remaining_assumptions: Vec<String>,
}

/// Prepared full statement plus each removal candidate bound to on-disk sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimizationPrepared {
    pub entry_id: &'static str,
    pub full_claim: Claim,
    pub full_formal_statement: FormalStatement,
    pub full_job: VerificationJob,
    pub full_source_path: PathBuf,
    pub full_source_bytes: Vec<u8>,
    pub trials: Vec<PreparedRemovalTrial>,
}

/// One removal candidate with content-addressed formal artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRemovalTrial {
    pub remove_index: usize,
    pub removed_assumption: &'static str,
    pub weakened_statement: &'static str,
    pub expected: ExpectedRemovalOutcome,
    pub claim: Claim,
    pub formal_statement: FormalStatement,
    pub job: VerificationJob,
    pub source_path: PathBuf,
    pub source_bytes: Vec<u8>,
}

/// Measured report for one removal trial after kernel re-verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalTrialReport {
    pub entry_id: &'static str,
    pub remove_index: usize,
    pub removed_assumption: &'static str,
    pub outcome: RemovalOutcome,
    pub expected: ExpectedRemovalOutcome,
    pub matches_expectation: bool,
    pub produced_proof: bool,
    /// Explicit non-claim: necessity is never inferred from rejection alone.
    pub necessity_claimed: bool,
}

/// Aggregate report for one minimization entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimizationRunReport {
    pub entry_id: &'static str,
    pub full_accepted: bool,
    pub full_produced_proof: bool,
    pub trials: Vec<RemovalTrialReport>,
}

/// Fail-closed assumption-minimization error.
#[derive(Debug)]
pub enum MinimizationError {
    Io(std::io::Error),
    MissingSource(PathBuf),
    JobConstruction(String),
    InvalidTrial {
        entry_id: &'static str,
        remove_index: usize,
    },
    Verification(VerificationError),
}

impl fmt::Display for MinimizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "minimization I/O error: {error}"),
            Self::MissingSource(path) => {
                write!(formatter, "minimization source missing: {}", path.display())
            }
            Self::JobConstruction(detail) => {
                write!(
                    formatter,
                    "minimization verification job construction failed: {detail}"
                )
            }
            Self::InvalidTrial {
                entry_id,
                remove_index,
            } => write!(
                formatter,
                "minimization entry {entry_id} has invalid removal index {remove_index}"
            ),
            Self::Verification(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for MinimizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::MissingSource(_) | Self::JobConstruction(_) | Self::InvalidTrial { .. } => None,
        }
    }
}

impl From<std::io::Error> for MinimizationError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<VerificationError> for MinimizationError {
    fn from(error: VerificationError) -> Self {
        Self::Verification(error)
    }
}

/// PL-1.2 control catalog: known proved statements with curated removal trials.
///
/// Matching expected accept/reject outcomes measures orchestration only and does
/// not claim mathematical novelty or assumption necessity.
pub const ASSUMPTION_MINIMIZATION_CORPUS: &[MinimizationEntry] = &[
    MinimizationEntry {
        id: "nat-rfl-drop-redundant-true",
        statement: "forall (n : Nat) (_unused : True), n = n",
        assumptions: &["n : Nat", "_unused : True"],
        relative_path_full: "ProofLab/Corpus/Minimize/NatRflRedundant.lean",
        imports: &["Mathlib.Data.Nat.Basic"],
        removal_trials: &[RemovalTrial {
            remove_index: 1,
            removed_assumption: "_unused : True",
            weakened_statement: "forall (n : Nat), n = n",
            relative_path: "ProofLab/Corpus/Minimize/NatRflDropTrue.lean",
            expected: ExpectedRemovalOutcome::KernelAccepts,
        }],
    },
    MinimizationEntry {
        id: "nat-eq-by-hyp-drop-h",
        statement: "forall (n : Nat) (h : n = 1), n = 1",
        assumptions: &["n : Nat", "h : n = 1"],
        relative_path_full: "ProofLab/Corpus/Minimize/NatEqByHyp.lean",
        imports: &["Mathlib.Data.Nat.Basic"],
        removal_trials: &[RemovalTrial {
            remove_index: 1,
            removed_assumption: "h : n = 1",
            weakened_statement: "forall (n : Nat), n = 1",
            relative_path: "ProofLab/Corpus/Fixtures/MinimizeDropHypReject.lean",
            expected: ExpectedRemovalOutcome::KernelRejects,
        }],
    },
];

/// Automatically enumerate leave-one-out assumption-removal candidates.
///
/// This is the pure PL-1.2 removal generator. Pairing a candidate with a Lean
/// fixture and re-verifying it is the responsibility of the corpus runner.
#[must_use]
pub fn leave_one_out_candidates(assumptions: &[String]) -> Vec<RemovalCandidate> {
    assumptions
        .iter()
        .enumerate()
        .map(|(remove_index, removed)| {
            let remaining_assumptions = assumptions
                .iter()
                .enumerate()
                .filter(|&(index, _)| index != remove_index)
                .map(|(_, assumption)| assumption.clone())
                .collect();
            RemovalCandidate {
                remove_index,
                removed_assumption: removed.clone(),
                remaining_assumptions,
            }
        })
        .collect()
}

impl MinimizationEntry {
    /// Build the immutable claim for the full (pre-minimization) statement.
    #[must_use]
    pub fn full_claim(&self) -> Claim {
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

    /// Automatically generate leave-one-out candidates from the entry assumptions.
    #[must_use]
    pub fn automatic_candidates(&self) -> Vec<RemovalCandidate> {
        let assumptions = self
            .assumptions
            .iter()
            .map(|assumption| (*assumption).to_owned())
            .collect::<Vec<_>>();
        leave_one_out_candidates(&assumptions)
    }

    /// Resolve a repository-relative path under `repo_root`.
    #[must_use]
    pub fn source_path(&self, repo_root: impl AsRef<Path>, relative: &str) -> PathBuf {
        repo_root.as_ref().join(relative)
    }

    /// Load full and candidate sources; build content-addressed jobs.
    ///
    /// # Errors
    ///
    /// Fails closed on missing sources, I/O errors, invalid trial indices, or
    /// incomplete reproducibility metadata.
    pub fn prepare(
        &self,
        repo_root: impl AsRef<Path>,
        repro: &ReproMeta,
    ) -> Result<MinimizationPrepared, MinimizationError> {
        let repo_root = repo_root.as_ref();
        let full_source_path = self.source_path(repo_root, self.relative_path_full);
        if !full_source_path.is_file() {
            return Err(MinimizationError::MissingSource(full_source_path));
        }
        let full_source_bytes = fs::read(&full_source_path)?;
        let full_claim = self.full_claim();
        let imports = self
            .imports
            .iter()
            .map(|import| (*import).to_owned())
            .collect::<Vec<_>>();
        let full_formal_statement =
            FormalStatement::lean4(full_claim.id, &full_source_bytes, imports.clone());
        let full_job = VerificationJob::new(
            &full_formal_statement,
            &full_source_bytes,
            LEAN_INVOCATION,
            repro.clone(),
        )
        .map_err(|error| MinimizationError::JobConstruction(error.to_string()))?;

        let mut trials = Vec::with_capacity(self.removal_trials.len());
        for trial in self.removal_trials {
            if trial.remove_index >= self.assumptions.len() {
                return Err(MinimizationError::InvalidTrial {
                    entry_id: self.id,
                    remove_index: trial.remove_index,
                });
            }
            if self.assumptions[trial.remove_index] != trial.removed_assumption {
                return Err(MinimizationError::InvalidTrial {
                    entry_id: self.id,
                    remove_index: trial.remove_index,
                });
            }
            let remaining = self
                .assumptions
                .iter()
                .enumerate()
                .filter(|&(index, _)| index != trial.remove_index)
                .map(|(_, assumption)| (*assumption).to_owned())
                .collect::<Vec<_>>();
            let source_path = self.source_path(repo_root, trial.relative_path);
            if !source_path.is_file() {
                return Err(MinimizationError::MissingSource(source_path));
            }
            let source_bytes = fs::read(&source_path)?;
            let claim = Claim::new(ClaimBody {
                statement: trial.weakened_statement.into(),
                assumptions: remaining,
                parents: vec![full_claim.id],
            });
            let formal_statement = FormalStatement::lean4(claim.id, &source_bytes, imports.clone());
            let job = VerificationJob::new(
                &formal_statement,
                &source_bytes,
                LEAN_INVOCATION,
                repro.clone(),
            )
            .map_err(|error| MinimizationError::JobConstruction(error.to_string()))?;
            trials.push(PreparedRemovalTrial {
                remove_index: trial.remove_index,
                removed_assumption: trial.removed_assumption,
                weakened_statement: trial.weakened_statement,
                expected: trial.expected,
                claim,
                formal_statement,
                job,
                source_path,
                source_bytes,
            });
        }

        Ok(MinimizationPrepared {
            entry_id: self.id,
            full_claim,
            full_formal_statement,
            full_job,
            full_source_path,
            full_source_bytes,
            trials,
        })
    }
}

impl MinimizationPrepared {
    /// Re-verify the full statement and each removal candidate through Lean.
    ///
    /// # Errors
    ///
    /// Propagates trust-boundary contract violations. Individual Lean
    /// accept/reject outcomes are captured in trial reports.
    pub fn verify(
        &self,
        kernel: &LeanKernel,
        observed: Option<&EnvironmentLock>,
    ) -> Result<(Vec<VerificationOutcome>, MinimizationRunReport), MinimizationError> {
        let full_outcome = verify_prepared(
            kernel,
            observed,
            &self.full_job,
            &self.full_formal_statement,
            &self.full_source_path,
        )?;
        let mut outcomes = vec![full_outcome.clone()];
        let mut trial_reports = Vec::with_capacity(self.trials.len());

        for trial in &self.trials {
            let outcome = verify_prepared(
                kernel,
                observed,
                &trial.job,
                &trial.formal_statement,
                &trial.source_path,
            )?;
            let produced_proof = outcome.proof.is_some();
            let removal_outcome = if outcome.result.accepted && produced_proof {
                RemovalOutcome::Removable
            } else {
                RemovalOutcome::RemovalRejected
            };
            trial_reports.push(RemovalTrialReport {
                entry_id: self.entry_id,
                remove_index: trial.remove_index,
                removed_assumption: trial.removed_assumption,
                outcome: removal_outcome,
                expected: trial.expected,
                matches_expectation: removal_outcome.matches_expectation(trial.expected),
                produced_proof,
                // Hard invariant: rejection never becomes a necessity claim.
                necessity_claimed: false,
            });
            outcomes.push(outcome);
        }

        Ok((
            outcomes,
            MinimizationRunReport {
                entry_id: self.entry_id,
                full_accepted: full_outcome.result.accepted,
                full_produced_proof: full_outcome.proof.is_some(),
                trials: trial_reports,
            },
        ))
    }
}

fn verify_prepared(
    kernel: &LeanKernel,
    observed: Option<&EnvironmentLock>,
    job: &VerificationJob,
    formal_statement: &FormalStatement,
    source_path: &Path,
) -> Result<VerificationOutcome, MinimizationError> {
    match observed {
        Some(lock) => Ok(kernel.verify_job_with_lock(job, formal_statement, source_path, lock)?),
        None => Ok(kernel.verify_job(job, formal_statement, source_path)?),
    }
}

/// Prepare every minimization control under `repo_root`.
///
/// # Errors
///
/// Fails closed on the first entry that cannot be prepared.
pub fn prepare_minimization_corpus(
    repo_root: impl AsRef<Path>,
    repro: &ReproMeta,
) -> Result<Vec<MinimizationPrepared>, MinimizationError> {
    let repo_root = repo_root.as_ref();
    ASSUMPTION_MINIMIZATION_CORPUS
        .iter()
        .map(|entry| entry.prepare(repo_root, repro))
        .collect()
}

/// Run the full PL-1.2 assumption-minimization battery.
///
/// # Errors
///
/// Fails closed on preparation or trust-boundary contract violations.
pub fn verify_assumption_minimization(
    kernel: &LeanKernel,
    repo_root: impl AsRef<Path>,
    repro: &ReproMeta,
    observed: Option<&EnvironmentLock>,
) -> Result<Vec<MinimizationRunReport>, MinimizationError> {
    prepare_minimization_corpus(repo_root, repro)?
        .into_iter()
        .map(|prepared| {
            let (_outcomes, report) = prepared.verify(kernel, observed)?;
            Ok(report)
        })
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
        let mut repro = ReproMeta::bootstrap("pl-1.2-assumption-minimization");
        repro.prooflab_revision = "minimize-unit-test".into();
        repro
    }

    #[test]
    fn leave_one_out_enumerates_every_index() {
        let assumptions = vec![
            "n : Nat".into(),
            "_unused : True".into(),
            "h : n = 1".into(),
        ];
        let candidates = leave_one_out_candidates(&assumptions);
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].remove_index, 0);
        assert_eq!(candidates[0].remaining_assumptions, assumptions[1..]);
        assert_eq!(
            candidates[1].remaining_assumptions,
            vec!["n : Nat".to_owned(), "h : n = 1".to_owned()]
        );
        assert_eq!(candidates[2].remaining_assumptions, assumptions[..2]);
    }

    #[test]
    fn corpus_covers_accept_and_reject_controls() {
        let accepts = ASSUMPTION_MINIMIZATION_CORPUS
            .iter()
            .flat_map(|entry| entry.removal_trials)
            .filter(|trial| trial.expected == ExpectedRemovalOutcome::KernelAccepts)
            .count();
        let rejects = ASSUMPTION_MINIMIZATION_CORPUS
            .iter()
            .flat_map(|entry| entry.removal_trials)
            .filter(|trial| trial.expected == ExpectedRemovalOutcome::KernelRejects)
            .count();
        assert!(
            accepts >= 1,
            "PL-1.2 needs at least one redundant-assumption accept control"
        );
        assert!(
            rejects >= 1,
            "PL-1.2 needs at least one removal-reject control"
        );
    }

    #[test]
    fn automatic_candidates_cover_catalogued_trials() {
        for entry in ASSUMPTION_MINIMIZATION_CORPUS {
            let automatic = entry.automatic_candidates();
            assert_eq!(automatic.len(), entry.assumptions.len());
            for trial in entry.removal_trials {
                let match_candidate = automatic
                    .iter()
                    .find(|candidate| candidate.remove_index == trial.remove_index)
                    .expect("catalogued trial must be an automatic leave-one-out candidate");
                assert_eq!(
                    match_candidate.removed_assumption, trial.removed_assumption,
                    "trial label must match automatic removal"
                );
            }
        }
    }

    #[test]
    fn prepared_entries_bind_on_disk_sources() {
        for entry in ASSUMPTION_MINIMIZATION_CORPUS {
            let prepared = entry.prepare(repo_root(), &repro()).expect("prepare");
            assert!(prepared.full_formal_statement.check_id());
            assert!(
                prepared
                    .full_formal_statement
                    .matches_source(&prepared.full_source_bytes)
            );
            assert!(prepared.full_job.check_id());
            assert!(
                prepared
                    .full_job
                    .matches_proof_source(&prepared.full_source_bytes)
            );
            assert!(prepared.full_source_path.is_file());
            for trial in &prepared.trials {
                assert!(trial.formal_statement.check_id());
                assert!(trial.formal_statement.matches_source(&trial.source_bytes));
                assert!(trial.job.check_id());
                assert!(trial.job.matches_proof_source(&trial.source_bytes));
                assert!(trial.source_path.is_file());
                assert_ne!(trial.claim.id, prepared.full_claim.id);
                assert_eq!(trial.claim.body.parents, vec![prepared.full_claim.id]);
            }
        }
    }

    #[test]
    fn removal_outcome_never_encodes_necessity() {
        assert!(!RemovalOutcome::Removable.matches_expectation(
            // Removable must not be confused with reject controls.
            ExpectedRemovalOutcome::KernelRejects
        ));
        assert!(
            RemovalOutcome::RemovalRejected
                .matches_expectation(ExpectedRemovalOutcome::KernelRejects)
        );
        // There is intentionally no RemovalOutcome::Necessary variant.
        let label = format!("{:?}", RemovalOutcome::RemovalRejected);
        assert!(
            !label.to_lowercase().contains("necess"),
            "rejection outcome must not be named as necessity"
        );
    }

    #[test]
    fn drifted_lock_fails_before_kernel_for_minimization_entry() {
        let prepared = ASSUMPTION_MINIMIZATION_CORPUS[0]
            .prepare(repo_root(), &repro())
            .expect("prepare");
        let observed = EnvironmentLock::new(
            "v4.33.1",
            "0df444a360eaa60ab8c11dca51a86af692955474",
            "minimize-unit-test",
            "different-environment",
        )
        .unwrap();
        let kernel = LeanKernel::new("this-command-must-not-run");
        assert!(matches!(
            prepared.verify(&kernel, Some(&observed)),
            Err(MinimizationError::Verification(VerificationError::Drift(_)))
        ));
    }

    #[test]
    fn claim_identity_changes_when_assumption_removed() {
        let entry = &ASSUMPTION_MINIMIZATION_CORPUS[0];
        let full = entry.full_claim();
        let prepared = entry.prepare(repo_root(), &repro()).expect("prepare");
        let weakened = &prepared.trials[0].claim;
        assert_ne!(full.id, weakened.id);
        assert_eq!(weakened.body.assumptions, vec!["n : Nat".to_owned()]);
    }
}
