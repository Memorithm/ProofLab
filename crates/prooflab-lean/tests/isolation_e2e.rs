#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use prooflab_lean::{BubblewrapIsolation, LeanIsolationLimits, LeanKernel};

fn write_isolation_probe(path: &Path, outside_secret: &Path) {
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
set -eu
test ! -e '{}'
if touch "$PWD/host-write"; then
  echo "read-only project write unexpectedly succeeded" >&2
  exit 71
fi
touch /tmp/prooflab-private-write
test "$1" = env
test "$2" = lean
test "$3" = /tmp/prooflab-source.lean
cmp "$3" "$PWD/Input.lean"
if (printf 'altered source\n' > "$3"); then
  echo "staged proof-source overwrite unexpectedly succeeded" >&2
  exit 73
fi
printf 'replacement source\n' > /tmp/prooflab-replacement
if mv /tmp/prooflab-replacement "$3"; then
  echo "staged proof-source replacement unexpectedly succeeded" >&2
  exit 74
fi
if rm "$3"; then
  echo "staged proof-source unlink unexpectedly succeeded" >&2
  exit 75
fi
cmp "$3" "$PWD/Input.lean"
echo source-immutability-ok
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
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

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
    let source_bytes = b"theorem input : True := trivial\n";
    fs::write(&source, source_bytes).unwrap();

    let outside_secret = root.join("outside-secret");
    fs::write(&outside_secret, "must not be visible").unwrap();
    let fake_lake = project.join("fake-lake");
    write_isolation_probe(&fake_lake, &outside_secret);

    let isolation = BubblewrapIsolation::new("/usr/bin/bwrap", "/usr/bin/prlimit").with_limits(
        LeanIsolationLimits {
            max_address_space_bytes: 512 * 1024 * 1024,
            max_cpu_seconds: 10,
            max_processes: 32,
            max_file_size_bytes: 1024 * 1024,
            max_open_files: 64,
        },
    );
    let kernel = LeanKernel::new(fake_lake.canonicalize().unwrap()).with_isolation(isolation);
    let result = kernel.verify_untrusted_file(&source).unwrap();

    assert!(
        result.accepted,
        "isolated fixture failed. stdout:\n{}\nstderr:\n{}",
        result.stdout, result.stderr
    );
    assert!(result.stdout.contains("source-immutability-ok"));
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert!(!project.join("host-write").exists());
    fs::remove_dir_all(root).unwrap();
}
