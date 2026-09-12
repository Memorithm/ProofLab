use std::fs;
use std::path::{Path, PathBuf};

use prooflab_core::{Claim, ClaimBody, FormalStatement, ReproMeta, VerificationJob};
use prooflab_lean::LeanKernel;
use prooflab_store::{MemoryProofStore, ProofStore};

const LEAN_INVOCATION: &str = "lake env lean";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rejected_fixture_path(root: &Path) -> PathBuf {
    let directory = root.join(".prooflab/e2e");
    fs::create_dir_all(&directory).unwrap();
    directory.join(format!("rejected-{}.lean", std::process::id()))
}

fn repro_meta() -> ReproMeta {
    let mut repro = ReproMeta::bootstrap("pl-0.2-pinned-ci-environment");
    repro.prooflab_revision =
        std::env::var("PROOFLAB_TEST_REVISION").unwrap_or_else(|_| "local-integration-test".into());
    repro
}

fn claim(statement: &str) -> Claim {
    Claim::new(ClaimBody {
        statement: statement.into(),
        assumptions: vec![],
        parents: vec![],
    })
}

#[test]
#[ignore = "requires the pinned Lean/mathlib environment"]
fn accepted_and_rejected_lean_paths_preserve_the_trust_boundary() {
    let root = repo_root();
    let accepted_path = root.join("ProofLab/Core/Smoke.lean");
    let accepted_source = fs::read(&accepted_path).unwrap();

    let rejected_source =
        b"import Mathlib\n\ntheorem prooflabE2ERejected : (2 : Nat) + 2 = 5 := by decide\n";
    let rejected_path = rejected_fixture_path(&root);
    fs::write(&rejected_path, rejected_source).unwrap();

    let accepted_formal = FormalStatement::lean4(
        claim("forall n : Nat, n = n").id,
        &accepted_source,
        vec!["Mathlib".into()],
    );
    let rejected_formal = FormalStatement::lean4(
        claim("2 + 2 = 5").id,
        rejected_source,
        vec!["Mathlib".into()],
    );

    let accepted_job = VerificationJob::new(
        &accepted_formal,
        &accepted_source,
        LEAN_INVOCATION,
        repro_meta(),
    )
    .unwrap();
    let rejected_job = VerificationJob::new(
        &rejected_formal,
        rejected_source,
        LEAN_INVOCATION,
        repro_meta(),
    )
    .unwrap();

    let kernel = LeanKernel::default();
    let accepted = kernel
        .verify_job(&accepted_job, &accepted_formal, &accepted_path)
        .unwrap();
    let rejected = kernel
        .verify_job(&rejected_job, &rejected_formal, &rejected_path)
        .unwrap();

    fs::remove_file(&rejected_path).unwrap();

    assert!(
        accepted.result.accepted,
        "Lean rejected the known-valid Smoke theorem. stdout:\n{}\nstderr:\n{}",
        accepted.result.stdout, accepted.result.stderr
    );
    assert_eq!(accepted.result.exit_code, Some(0));
    let proof = accepted
        .proof
        .expect("an accepted Lean theorem must produce a proof artifact");
    assert!(proof.check_id());
    assert_eq!(proof.body.formal_statement_id, accepted_job.formal_statement_id);
    assert_eq!(proof.body.proof_source_digest, accepted_job.proof_source_digest);
    assert_eq!(proof.body.dependencies, accepted_job.dependencies);
    assert_eq!(proof.body.repro, accepted_job.repro);

    let mut store = MemoryProofStore::default();
    let stored_id = store.put(&proof).unwrap();
    assert_eq!(stored_id, proof.id);
    assert_eq!(store.get(proof.id).unwrap(), Some(proof));

    assert!(
        !rejected.result.accepted,
        "Lean unexpectedly accepted the false theorem. stdout:\n{}\nstderr:\n{}",
        rejected.result.stdout, rejected.result.stderr
    );
    assert_ne!(rejected.result.exit_code, Some(0));
    assert!(
        rejected.proof.is_none(),
        "a rejected Lean theorem must never produce a proof artifact"
    );
}
