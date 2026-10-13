//! Qualify the real importer through the existing, independently configured owner.
//! The owner executable and key policy are trusted operator inputs, not payloads.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use prooflab_core::{
    BenchExecutionManifest, Claim, ClaimStatus, ingest_bench_execution, sha256_bytes,
};

fn owner(
    executable: &Path,
    workspace: &Path,
    args: &[&str],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let result = Command::new(executable)
        .args(args)
        .current_dir(workspace)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()?;
    if !result.status.success() || result.stdout.len() > 262_144 || result.stderr.len() > 262_144 {
        return Err("trusted owner invocation refused or exceeded the response contract".into());
    }
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn seal_and_authenticate(
    executable: &Path,
    workspace: &Path,
    key: &Path,
    trust: &Path,
) -> Result<(String, serde_json::Value), Box<dyn std::error::Error>> {
    let verified = owner(
        executable,
        workspace,
        &[
            "verify",
            workspace.to_str().ok_or("UTF-8 workspace required")?,
            "--json",
        ],
    )?;
    let run = verified["run_id"]
        .as_str()
        .ok_or("owner run identity absent")?;
    if verified["overall_verdict"] != "PASS" {
        return Err("owner record sealing failed".into());
    }
    let principal = "prooflab-development-qualification";
    owner(
        executable,
        workspace,
        &[
            "attest",
            run,
            "--principal",
            principal,
            "--identity",
            key.to_str().ok_or("UTF-8 signer path required")?,
            "--json",
        ],
    )?;
    let envelope = workspace
        .join(".scirust-verify/attestations")
        .join(format!("{run}.json"));
    let receipt = owner(
        executable,
        workspace,
        &[
            "authenticate",
            run,
            "--envelope",
            envelope.to_str().ok_or("UTF-8 envelope path required")?,
            "--allowed-signers",
            trust.to_str().ok_or("UTF-8 trust path required")?,
            "--json",
        ],
    )?;
    if receipt["authenticity"]["status"] != "verified"
        || receipt["integrity"]["status"] != "verified"
        || receipt["authenticity"]["principal"] != principal
        || receipt["run_id"] != run
    {
        return Err("owner authentication did not establish the configured identity".into());
    }
    Ok((run.to_owned(), receipt))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if args.len() != 4 {
        return Err(
            "expected owner binary, staged run, signer identity and external allowed-signers"
                .into(),
        );
    }
    let executable = args[0].canonicalize()?;
    let workspace = args[1].canonicalize()?;
    let key = args[2].canonicalize()?;
    let trust = args[3].canonicalize()?;
    let (run, receipt) = seal_and_authenticate(&executable, &workspace, &key, &trust)?;
    let folder = workspace.join(".scirust-verify/runs").join(&run);
    let bundle_bytes = fs::read(folder.join("bundle.json"))?;
    let bundle: serde_json::Value = serde_json::from_slice(&bundle_bytes)?;
    let bundle_hash = hex(&sha256_bytes(&bundle_bytes));
    if receipt["bundle_sha256"] != bundle_hash {
        return Err("authenticated bundle bytes changed".into());
    }
    let record_name = "logs/custom-development-execution-record.out.log";
    let record = fs::read(folder.join(record_name))?;
    if bundle["files"][record_name] != hex(&sha256_bytes(&record)) {
        return Err("sealed execution record bytes changed".into());
    }
    let manifest: BenchExecutionManifest = serde_json::from_slice(&record)?;
    // The identity comes from the freshly authenticated sealed owner attachment,
    // never from a producer boolean or an unauthenticated JSON receipt.
    let expected_id = manifest.checked_id()?;
    let claim: Claim = serde_json::from_slice(&fs::read(workspace.join("claim.json"))?)?;
    if Claim::new(claim.body.clone()).id != claim.id {
        return Err("claim content address mismatch".into());
    }
    let sources = std::iter::once(&manifest.protocol)
        .chain(manifest.inputs.iter())
        .chain(manifest.development_splits.iter())
        .map(|input| {
            Ok((
                input.name.clone(),
                fs::read(workspace.join("sources").join(&input.name))?,
            ))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    let payload = fs::read(workspace.join("ci-job.log"))?;
    let allowed: BTreeSet<String> = ["development/main-constants".into()].into_iter().collect();
    let result = ingest_bench_execution(
        claim.id,
        &manifest,
        expected_id,
        &allowed,
        "release-smoke-log",
        &payload,
        &sources,
    )?;
    if result.evidence.status != ClaimStatus::Observed {
        return Err("empirical status changed".into());
    }
    fs::write(
        workspace.join("ingested.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    fs::write(
        workspace.join("owner-authentication.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    println!(
        "authenticated execution import: {}; status=Observed; qualification=not_evaluated",
        hex(&expected_id)
    );
    Ok(())
}

fn hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(64);
    for byte in bytes {
        write!(&mut text, "{byte:02x}").expect("String write cannot fail");
    }
    text
}
