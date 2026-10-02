#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use prooflab_lean::{BubblewrapIsolation, CgroupV2Policy, LeanIsolationLimits, LeanKernel};

fn delegated_cgroup_root() -> std::path::PathBuf {
    std::env::var_os("PROOFLAB_CGROUP_ROOT")
        .map(std::path::PathBuf::from)
        .expect("PROOFLAB_CGROUP_ROOT must name an empty delegated cgroup v2 subtree")
}

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
test "$3" = -j
test "$4" = 1
test "$5" = /tmp/prooflab-source.lean
cmp "$5" "$PWD/Input.lean"
if (printf 'altered source\n' > "$5"); then
  echo "staged proof-source overwrite unexpectedly succeeded" >&2
  exit 73
fi
printf 'replacement source\n' > /tmp/prooflab-replacement
if mv /tmp/prooflab-replacement "$5"; then
  echo "staged proof-source replacement unexpectedly succeeded" >&2
  exit 74
fi
if rm "$5"; then
  echo "staged proof-source unlink unexpectedly succeeded" >&2
  exit 75
fi
cmp "$5" "$PWD/Input.lean"
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

    let isolation = BubblewrapIsolation::new("/usr/bin/bwrap", "/usr/bin/prlimit")
        .with_limits(LeanIsolationLimits {
            max_source_bytes: 1024 * 1024,
            max_address_space_bytes: 512 * 1024 * 1024,
            max_cpu_seconds: 10,
            max_processes: 32,
            max_lean_threads: 1,
            max_file_size_bytes: 1024 * 1024,
            max_open_files: 64,
        })
        .with_cgroup_v2(CgroupV2Policy::new(delegated_cgroup_root()).with_limits(
            512 * 1024 * 1024,
            32,
            100_000,
            100_000,
        ));
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

#[test]
#[ignore = "requires Linux bubblewrap, prlimit and a delegated cgroup v2 subtree"]
fn aggregate_cgroup_rejects_process_fanout_and_is_cleaned() {
    assert_eq!(
        std::env::var("PROOFLAB_RUN_ISOLATION_E2E").as_deref(),
        Ok("1"),
        "the ignored isolation gate must only run when explicitly enabled"
    );

    let root = std::env::temp_dir().join(format!(
        "prooflab-cgroup-fanout-e2e-{}",
        std::process::id()
    ));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("lakefile.lean"), "").unwrap();
    let source = project.join("Input.lean");
    fs::write(&source, b"theorem input : True := trivial\n").unwrap();
    let fake_lake = project.join("fake-lake");
    fs::write(
        &fake_lake,
        r#"#!/bin/sh
set -eu
i=0
while [ "$i" -lt 64 ]; do
  /bin/sleep 30 &
  i=$((i + 1))
done
wait
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_lake).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_lake, permissions).unwrap();

    let cgroup_root = delegated_cgroup_root();
    let isolation = BubblewrapIsolation::new("/usr/bin/bwrap", "/usr/bin/prlimit")
        .with_limits(LeanIsolationLimits {
            max_processes: 1024,
            ..LeanIsolationLimits::default()
        })
        .with_cgroup_v2(CgroupV2Policy::new(&cgroup_root).with_limits(
            512 * 1024 * 1024,
            8,
            100_000,
            100_000,
        ));
    let kernel = LeanKernel::new(fake_lake.canonicalize().unwrap()).with_isolation(isolation);
    let result = kernel.verify_untrusted_file(&source).unwrap();

    assert!(!result.accepted, "aggregate PID fanout unexpectedly succeeded");
    assert!(result.exit_code.is_some_and(|code| code != 0));
    assert!(
        result.stderr.contains("Resource temporarily unavailable")
            || result.stderr.contains("Cannot fork")
            || result.stderr.contains("can't fork"),
        "expected a kernel-enforced fork rejection, stderr was: {}",
        result.stderr
    );
    fs::remove_dir_all(root).unwrap();
}
