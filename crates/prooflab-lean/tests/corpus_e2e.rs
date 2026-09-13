//! PL-1.0 known-theorem corpus integration gate.
//!
//! Runs only under the pinned Lean CI environment (`--ignored`). Measures
//! verification/orchestration correctness for already-known theorems and one
//! intentional falsehood. Does not claim mathematical novelty.

use std::collections::BTreeSet;
use std::path::PathBuf;

use prooflab_core::{EnvironmentLock, ReproMeta};
use prooflab_lean::{ExpectedOutcome, LeanKernel, prepare_corpus};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repro_meta() -> ReproMeta {
    let mut repro = ReproMeta::bootstrap("pl-1.0-known-theorem-corpus");
    repro.prooflab_revision =
        std::env::var("PROOFLAB_TEST_REVISION").unwrap_or_else(|_| "local-integration-test".into());
    repro
}

#[test]
#[ignore = "requires the pinned Lean/mathlib environment"]
fn known_theorem_corpus_matches_accept_and_reject_expectations() {
    let repro = repro_meta();
    let observed = EnvironmentLock::from_repro(&repro).expect("lock from repro");
    let prepared = prepare_corpus(repo_root(), &repro).expect("prepare corpus");
    assert!(
        prepared.len() >= 5,
        "expected the minimal PL-1.0 corpus size, got {}",
        prepared.len()
    );

    let kernel = LeanKernel::default();
    let mut seen_shapes = BTreeSet::new();
    let mut accepts = 0usize;
    let mut rejects = 0usize;

    for entry in &prepared {
        let (outcome, report) = entry
            .verify(&kernel, Some(&observed))
            .unwrap_or_else(|error| {
                panic!("corpus entry {} failed closed: {error}", entry.entry_id)
            });

        assert!(
            report.matches_expectation,
            "corpus entry {} violated expectation {:?}: accepted={}, proof={}, stdout:\n{}\nstderr:\n{}",
            report.entry_id,
            report.expected,
            report.accepted,
            report.produced_proof,
            outcome.result.stdout,
            outcome.result.stderr
        );

        match report.expected {
            ExpectedOutcome::Accept => {
                accepts += 1;
                let proof = outcome
                    .proof
                    .expect("accept entries must seal a ProofArtifact");
                assert!(proof.check_id());
                assert_eq!(proof.body.formal_statement_id, entry.formal_statement.id);
                assert_eq!(
                    proof.body.proof_source_digest,
                    entry.job.proof_source_digest
                );
                assert_eq!(proof.body.repro, repro);
                // Recover the catalogued proof shape for diversity accounting.
                let shape = prooflab_lean::KNOWN_THEOREM_CORPUS
                    .iter()
                    .find(|catalog| catalog.id == entry.entry_id)
                    .map(|catalog| catalog.proof_shape)
                    .expect("prepared entry must remain in the catalog");
                seen_shapes.insert(shape);
            }
            ExpectedOutcome::Reject => {
                rejects += 1;
                assert!(
                    outcome.proof.is_none(),
                    "reject fixture must never produce a ProofArtifact"
                );
            }
        }
    }

    assert!(accepts >= 3, "need several accept fixtures, got {accepts}");
    assert_eq!(rejects, 1, "need exactly one intentional reject fixture");
    assert!(
        seen_shapes.len() >= 3,
        "accept fixtures must exercise distinct proof shapes, got {seen_shapes:?}"
    );
}
