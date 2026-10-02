#![cfg(target_os = "linux")]

use std::fs;
use std::path::PathBuf;

use prooflab_lean::{BubblewrapIsolation, LeanIsolationLimits, LeanKernel};

#[test]
#[ignore = "requires the pinned Lean toolchain and administrator-owned setuid Bubblewrap"]
fn isolated_api_accepts_and_rejects_with_pinned_lean() {
    if std::env::var_os("PROOFLAB_RUN_ISOLATED_LEAN_E2E").is_none() {
        return;
    }

    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let repository = repository.canonicalize().unwrap();
    let lake = PathBuf::from(
        std::env::var_os("PROOFLAB_LAKE_BINARY")
            .expect("PROOFLAB_LAKE_BINARY must name the pinned Lake executable"),
    );
    let runtime_root = PathBuf::from(
        std::env::var_os("PROOFLAB_LEAN_RUNTIME_ROOT")
            .expect("PROOFLAB_LEAN_RUNTIME_ROOT must name the pinned toolchain root"),
    );
    let qualification_limits = LeanIsolationLimits {
        max_address_space_bytes: 8 * 1024 * 1024 * 1024,
        max_processes: 1024,
        max_lean_threads: 1,
        ..LeanIsolationLimits::default()
    };
    let kernel = LeanKernel::new(lake).with_isolation(
        BubblewrapIsolation::new("/usr/bin/bwrap", "/usr/bin/prlimit")
            .with_runtime_root(runtime_root)
            .with_limits(qualification_limits),
    );

    let fixture_root = repository.join("target/prooflab-isolated-lean-e2e");
    fs::create_dir_all(&fixture_root).unwrap();
    let accepted_source = fixture_root.join("Accepted.lean");
    fs::write(
        &accepted_source,
        "theorem input : True := by\n  exact True.intro\n",
    )
    .unwrap();
    let accepted = kernel.verify_untrusted_file(&accepted_source).unwrap();
    assert!(
        accepted.accepted,
        "pinned Lean rejected the valid source: {}",
        accepted.stderr
    );

    let rejected_source = fixture_root.join("Rejected.lean");
    fs::write(
        &rejected_source,
        "theorem impossible : False := by\n  exact True.intro\n",
    )
    .unwrap();
    let rejected = kernel.verify_untrusted_file(&rejected_source).unwrap();
    assert!(!rejected.accepted);
    assert!(rejected.exit_code.is_some_and(|code| code != 0));
    fs::remove_dir_all(fixture_root).unwrap();
}
