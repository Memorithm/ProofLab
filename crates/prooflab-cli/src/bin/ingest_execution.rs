//! Offline, bounded development execution importer. No execution or proof authority.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use prooflab_core::{BenchExecutionManifest, Claim, ingest_bench_execution};

#[derive(Debug, Parser)]
#[command(about = "Import pinned development execution bytes as empirical evidence only")]
struct Args {
    #[arg(long)]
    claim: PathBuf,
    #[arg(long)]
    manifest: PathBuf,
    /// Independent operator/owner-authenticated content pin. Never self-derive it.
    #[arg(long)]
    expected_execution_id: String,
    #[arg(long, required = true)]
    allow_development_split: Vec<String>,
    /// Trusted, staged protocol/input/split files named exactly as in the record.
    #[arg(long)]
    sources: PathBuf,
    #[arg(long)]
    entry_id: String,
    #[arg(long)]
    payload: PathBuf,
}

fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let metadata = path
        .symlink_metadata()
        .map_err(|_| "cannot inspect artifact")?;
    if !metadata.is_file() || metadata.len() > u64::try_from(limit).map_err(|_| "limit overflow")? {
        return Err("artifact is not a bounded regular file".into());
    }
    let file = File::open(path).map_err(|_| "cannot open artifact")?;
    let mut bytes = Vec::new();
    file.take(u64::try_from(limit + 1).map_err(|_| "limit overflow")?)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read artifact")?;
    if bytes.len() > limit {
        return Err("artifact exceeded its byte limit while reading".into());
    }
    Ok(bytes)
}

fn source_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    let path = Path::new(name);
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("noncanonical source artifact name".into());
    }
    let mut target = root.to_path_buf();
    for part in path.components() {
        target.push(part);
        if target
            .symlink_metadata()
            .map_err(|_| "missing source artifact")?
            .file_type()
            .is_symlink()
        {
            return Err("source artifacts may not traverse symlinks".into());
        }
    }
    Ok(target)
}

fn parse_pin(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("execution pin must be 64 lowercase hex characters".into());
    }
    let mut pin = [0; 32];
    for (index, byte) in pin.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "invalid execution pin")?;
    }
    Ok(pin)
}

fn ingest(args: &Args) -> Result<(), String> {
    let claim: Claim = serde_json::from_slice(&bounded_read(&args.claim, 256 * 1024)?)
        .map_err(|_| "invalid claim")?;
    if Claim::new(claim.body.clone()).id != claim.id {
        return Err("claim content address mismatch".into());
    }
    let manifest: BenchExecutionManifest =
        serde_json::from_slice(&bounded_read(&args.manifest, 256 * 1024)?)
            .map_err(|_| "invalid execution manifest")?;
    let pin = parse_pin(&args.expected_execution_id)?;
    if manifest.checked_id().map_err(str::to_owned)? != pin {
        return Err("independent execution content pin mismatch".into());
    }
    let allowed = args
        .allow_development_split
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if manifest
        .development_splits
        .iter()
        .any(|split| !allowed.contains(&split.name))
    {
        return Err("development split not authorized".into());
    }
    let root = args
        .sources
        .canonicalize()
        .map_err(|_| "invalid staged source root")?;
    let mut sources = Vec::new();
    let mut remaining = 64 * 1024 * 1024;
    let payload = bounded_read(&args.payload, remaining)?;
    remaining -= payload.len();
    for input in std::iter::once(&manifest.protocol)
        .chain(manifest.inputs.iter())
        .chain(manifest.development_splits.iter())
    {
        let bytes = bounded_read(&source_path(&root, &input.name)?, remaining)?;
        remaining -= bytes.len();
        sources.push((input.name.clone(), bytes));
    }
    let result = ingest_bench_execution(
        claim.id,
        &manifest,
        pin,
        &allowed,
        &args.entry_id,
        &payload,
        &sources,
    )
    .map_err(str::to_owned)?;
    println!(
        "{}",
        serde_json::to_string(&result).map_err(|_| "cannot encode empirical evidence")?
    );
    Ok(())
}

fn main() -> ExitCode {
    match ingest(&Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("execution ingest refused: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_paths_before_file_access_and_malformed_pins() {
        for name in ["../holdout", "/absolute/path", "./input"] {
            assert!(source_path(Path::new("/nonexistent"), name).is_err());
        }
        for pin in ["", "a", &"A".repeat(64), &"z".repeat(64)] {
            assert!(parse_pin(pin).is_err());
        }
        assert_eq!(parse_pin(&"01".repeat(32)).unwrap(), [1; 32]);
    }
}
