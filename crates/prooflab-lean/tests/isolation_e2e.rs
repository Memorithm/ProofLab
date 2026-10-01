#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;

use prooflab_lean::{BubblewrapIsolation, LeanIsolationLimits, LeanKernel};

#[test]
#[ignore = "requires Linux bubblewrap and prlimit"]
fn untrusted_boundary_enforces_filesystem_network_and_resource_policy() {
    assert_eq!(
        std::env::var("PROOFLAB_RUN_ISOLATION_E2E").as_deref(),
        Ok("1"),
        "the ignored isolation gate must only run when explicitly enabled"
    );

    let root = std::env::temp_dir().join(format!("prooflab-isolation-e2e-{}", std::process::id()));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("lakefile.lean"), "").unwrap();
    let source = project.join("Input.lean");
    fs::write(&source, "theorem input : True := trivial\n").unwrap();

    let outside_secret = root.join("outside-secret");
    fs::write(&outside_secret, "must not be visible").unwrap();
    let fake_lake = project.join("fake-lake");
    fs::write(
        &fake_lake,
        format!(
            r#"#!/bin/sh
set -eu
test ! -e '{}'
if touch "$PWD/host-write"; then
  echo "read-only project write unexpectedly succeeded" >&2
  exit 71
fi
touch /tmp/prooflab-private-write
if grep -Eq '^[[:space:]]*(eth|en|wl)[^:]*:' /proc/net/dev; then
  echo "non-loopback network interface is visible" >&2
  exit 72
fi
grep -E '^CapEff:[[:space:]]+0+$' /proc/self/status >/dev/null
grep -E 'Max processes[[:space:]]+32[[:space:]]+32' /proc/self/limits >/dev/null
"#,
            outside_secret.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_lake).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_lake, permissions).unwrap();

    let isolation = BubblewrapIsolation::new(
        "/usr/bin/unshare",
        "/usr/bin/bwrap",
        "/usr/bin/prlimit",
    )
    .with_limits(LeanIsolationLimits {
        max_address_space_bytes: 512 * 1024 * 1024,
        max_cpu_seconds: 10,
        max_processes: 32,
        max_file_size_bytes: 1024 * 1024,
        max_open_files: 64,
    });
    let kernel = LeanKernel::new(fake_lake.canonicalize().unwrap()).with_isolation(isolation);
    let result = kernel.verify_untrusted_file(&source).unwrap();

    assert!(
        result.accepted,
        "isolated fixture failed. stdout:\n{}\nstderr:\n{}",
        result.stdout, result.stderr
    );
    assert!(!project.join("host-write").exists());
    fs::remove_dir_all(root).unwrap();
}
