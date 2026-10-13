//! Prepare a real, previously executed public Riemann development run for import.
//! The caller must subsequently authenticate the record through the owner trust
//! contract before passing its content pin to the importer.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use prooflab_core::{
    BenchExecutionManifest, BenchExportEntry, BenchExportManifest, BenchKind, BenchSourceLabel,
    Claim, ClaimBody, ExecutionInput, sha256_bytes,
};

const SOURCE: &str = "d7f0b70feb515a3f3a80cd681d820510bc2ed938";
const RUN: u64 = 37_193_369_605;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let source = PathBuf::from(arguments.next().ok_or("source checkout required")?);
    let payload_path = PathBuf::from(arguments.next().ok_or("retained CI job log required")?);
    let output = PathBuf::from(arguments.next().ok_or("output directory required")?);
    if arguments.next().is_some() {
        return Err("unexpected argument".into());
    }
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&source)
        .output()?;
    if !head.status.success() || std::str::from_utf8(&head.stdout)?.trim() != SOURCE {
        return Err("Riemann source revision mismatch".into());
    }
    let payload = fs::read(payload_path)?;
    let text = std::str::from_utf8(&payload)?;
    if !text.contains(SOURCE)
        || !text.contains("Riemann N-dimensional bench - phase 2")
        || !text.contains("synthetic 2x2 spectrum =")
    {
        return Err("retained CI log lacks the pinned checkout or executed observations".into());
    }
    let sources = [
        (
            "protocol/workflow",
            fs::read(source.join(".github/workflows/ci.yml"))?,
        ),
        ("input/cargo-lock", fs::read(source.join("Cargo.lock"))?),
        ("input/main", fs::read(source.join("src/main.rs"))?),
        (
            "development/main-constants",
            fs::read(source.join("src/main.rs"))?,
        ),
    ];
    let identity = |index: usize| ExecutionInput {
        name: sources[index].0.into(),
        sha256: sha256_bytes(&sources[index].1),
    };
    let manifest = BenchExecutionManifest {
        schema_version: 1,
        export: BenchExportManifest::new(
            BenchKind::Riemann,
            "Memorithm/riemann_ndim_bench",
            SOURCE,
            "ci-job-111410052824",
            vec![BenchExportEntry::from_payload(
                "release-smoke-log",
                BenchSourceLabel::Numerical,
                "Published finite phase-2 numerical observations; no Riemann-hypothesis proof",
                "ci-job.log",
                &payload,
            )],
        )?,
        protocol: identity(0),
        inputs: vec![identity(1), identity(2)],
        development_splits: vec![identity(3)],
        ci_run_id: RUN,
        ci_attempt: 1,
        ci_url: format!("https://github.com/Memorithm/riemann_ndim_bench/actions/runs/{RUN}"),
    };
    let claim = Claim::new(ClaimBody {
        statement: "Public phase-2 finite numerical observation from an exact CI run".into(),
        assumptions: vec![
            "Finite declared probes only; published CI log is producer evidence".into(),
        ],
        parents: vec![],
    });
    fs::create_dir_all(&output)?;
    for (name, bytes) in &sources {
        let path = output.join("sources").join(name);
        fs::create_dir_all(path.parent().ok_or("source parent missing")?)?;
        fs::write(path, bytes)?;
    }
    fs::write(output.join("ci-job.log"), &payload)?;
    fs::write(
        output.join("execution.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        output.join("claim.json"),
        serde_json::to_vec_pretty(&claim)?,
    )?;
    let mut hex = String::with_capacity(64);
    for byte in manifest.checked_id()? {
        write!(&mut hex, "{byte:02x}")?;
    }
    fs::write(output.join("execution-id.txt"), format!("{hex}\n"))?;
    println!("prepared executed Riemann development evidence: {hex}");
    println!("Authenticate the exact execution record independently before import.");
    Ok(())
}
