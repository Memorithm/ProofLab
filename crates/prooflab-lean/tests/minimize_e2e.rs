//! PL-1.2 assumption-minimization integration gate.
//!
//! Runs only under the pinned Lean CI environment (`--ignored`). Measures
//! automatic assumption removal followed by kernel re-verification for known
//! proved controls. Does not claim mathematical novelty or assumption necessity.

use std::path::PathBuf;

use prooflab_core::{EnvironmentLock, ReproMeta};
use prooflab_lean::{
    ExpectedRemovalOutcome, LeanKernel, RemovalOutcome, prepare_minimization_corpus,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repro_meta() -> ReproMeta {
    let mut repro = ReproMeta::bootstrap("pl-1.2-assumption-minimization");
    repro.prooflab_revision =
        std::env::var("PROOFLAB_TEST_REVISION").unwrap_or_else(|_| "local-integration-test".into());
    repro
}

#[test]
#[ignore = "requires the pinned Lean/mathlib environment"]
fn assumption_minimization_matches_accept_and_reject_controls() {
    let repro = repro_meta();
    let observed = EnvironmentLock::from_repro(&repro).expect("lock from repro");
    let prepared = prepare_minimization_corpus(repo_root(), &repro).expect("prepare");
    assert!(
        prepared.len() >= 2,
        "expected the minimal PL-1.2 minimization corpus, got {}",
        prepared.len()
    );

    let kernel = LeanKernel::default();
    let mut removable = 0usize;
    let mut rejected = 0usize;

    for entry in &prepared {
        let (_outcomes, report) = entry
            .verify(&kernel, Some(&observed))
            .unwrap_or_else(|error| {
                panic!(
                    "minimization entry {} failed closed: {error}",
                    entry.entry_id
                )
            });

        assert!(
            report.full_accepted && report.full_produced_proof,
            "full statement {} must be proved before minimization trials",
            report.entry_id
        );

        for trial in &report.trials {
            assert!(
                !trial.necessity_claimed,
                "trial on {} must never claim necessity",
                trial.removed_assumption
            );
            assert!(
                trial.matches_expectation,
                "trial removing {} on {} violated expectation {:?}: got {:?}",
                trial.removed_assumption, trial.entry_id, trial.expected, trial.outcome
            );
            match trial.outcome {
                RemovalOutcome::Removable => {
                    removable += 1;
                    assert_eq!(trial.expected, ExpectedRemovalOutcome::KernelAccepts);
                    assert!(trial.produced_proof);
                }
                RemovalOutcome::RemovalRejected => {
                    rejected += 1;
                    assert_eq!(trial.expected, ExpectedRemovalOutcome::KernelRejects);
                    assert!(!trial.produced_proof);
                }
            }
        }
    }

    assert!(
        removable >= 1,
        "need at least one Removable control, got {removable}"
    );
    assert!(
        rejected >= 1,
        "need at least one RemovalRejected control, got {rejected}"
    );
}
