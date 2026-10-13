//! Development-only execution provenance above the existing bench export.
//!
//! A caller-supplied independent content pin binds the record. It is not a
//! signature, a trusted execution verdict, or a proof. Authentication belongs
//! to the existing SciRust-Verify/Hub trust boundary; never derive the pin from
//! the untrusted manifest itself and describe it as authentication.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::{
    BenchExportManifest, BenchSourceLabel, Canonical, CanonicalEncoder, ClaimId, EvidenceClaim,
    Observation, sha256_bytes, sha256_canonical,
};

const DOMAIN: &[u8] = b"prooflab:bench-development-execution:v1\0";
const MAX_ITEMS: usize = 64;
const MAX_TEXT: usize = 1024;
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

/// Exact identity of one protocol, input or split artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionInput {
    pub name: String,
    pub sha256: [u8; 32],
}

impl Canonical for ExecutionInput {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.name);
        encoder.value(&self.sha256);
    }
}

/// A producing bench's executed development record, with no proof authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchExecutionManifest {
    pub schema_version: u64,
    pub export: BenchExportManifest,
    pub protocol: ExecutionInput,
    /// Canonically sorted, distinct input identities.
    pub inputs: Vec<ExecutionInput>,
    /// Development split names and exact bytes; no final-holdout ingestion.
    pub development_splits: Vec<ExecutionInput>,
    pub ci_run_id: u64,
    pub ci_attempt: u64,
    pub ci_url: String,
}

impl BenchExecutionManifest {
    /// Validate bounds before canonical hashing or cloning.
    ///
    /// # Errors
    /// Returns a static diagnostic on malformed or noncanonical provenance.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 1 || self.ci_run_id == 0 || self.ci_attempt == 0 {
            return Err("unsupported execution identity");
        }
        if self.export.entries.len() > MAX_ITEMS
            || self.export.source_repository.len() > MAX_TEXT
            || self.export.source_revision.len() > 64
            || self.export.export_id.len() > MAX_TEXT
            || self.export.entries.iter().any(|entry| {
                [
                    entry.entry_id.len(),
                    entry.statement.len(),
                    entry.payload_ref.len(),
                ]
                .into_iter()
                .any(|length| length > MAX_TEXT)
            })
        {
            return Err("execution export exceeds bounds");
        }
        self.export.validate().map_err(|_| "invalid bench export")?;
        let repository = &self.export.source_repository;
        let expected_repository = match self.export.bench {
            crate::BenchKind::Tdi => "Memorithm/TDI",
            crate::BenchKind::Riemann => "Memorithm/riemann_ndim_bench",
        };
        if repository != expected_repository {
            return Err("bench and repository mismatch");
        }
        if self.ci_url
            != format!(
                "https://github.com/{repository}/actions/runs/{}",
                self.ci_run_id
            )
        {
            return Err("CI URL and run identity mismatch");
        }
        validate_inputs(std::slice::from_ref(&self.protocol))?;
        validate_inputs(&self.inputs)?;
        validate_inputs(&self.development_splits)?;
        if self
            .development_splits
            .iter()
            .any(|input| !input.name.starts_with("development/"))
        {
            return Err("only explicit development splits are admitted");
        }
        Ok(())
    }

    /// Bounded canonical identity, covering every provenance field.
    ///
    /// # Errors
    /// Rejects invalid records before hashing.
    pub fn checked_id(&self) -> Result<[u8; 32], &'static str> {
        self.validate()?;
        Ok(sha256_canonical(DOMAIN, self))
    }
}

fn validate_inputs(inputs: &[ExecutionInput]) -> Result<(), &'static str> {
    if inputs.is_empty() || inputs.len() > MAX_ITEMS {
        return Err("invalid execution input count");
    }
    let mut previous: Option<&str> = None;
    for input in inputs {
        if input.name.is_empty() || input.name.len() > MAX_TEXT || input.name.trim() != input.name {
            return Err("invalid execution input name");
        }
        if previous.is_some_and(|previous| previous >= input.name.as_str()) {
            return Err("execution inputs must be unique and canonically ordered");
        }
        previous = Some(&input.name);
    }
    Ok(())
}

impl Canonical for BenchExecutionManifest {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.u64(self.schema_version);
        encoder.value(&self.export);
        encoder.value(&self.protocol);
        encoder.seq(&self.inputs);
        encoder.seq(&self.development_splits);
        encoder.u64(self.ci_run_id);
        encoder.u64(self.ci_attempt);
        encoder.value(&self.ci_url);
    }
}

/// Provenance-bound observation and empirical evidence; no proof artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BenchExecutionIngest {
    pub execution_id: [u8; 32],
    pub source_label: BenchSourceLabel,
    pub observation: Observation,
    pub evidence: EvidenceClaim,
}

/// Ingest exact output bytes only under an independently supplied execution pin.
///
/// `allowed_development_splits` belongs to caller policy, never to the producer.
/// Input/split/protocol bytes are checked too. No unsigned self-pin is accepted
/// as an authenticated producer receipt by this API.
///
/// # Errors
/// Fails closed on any provenance/payload change or unauthorized split name.
pub fn ingest_bench_execution(
    claim_id: ClaimId,
    manifest: &BenchExecutionManifest,
    expected_execution_id: [u8; 32],
    allowed_development_splits: &BTreeSet<String>,
    entry_id: &str,
    payload: &[u8],
    source_bytes: &[(String, Vec<u8>)],
) -> Result<BenchExecutionIngest, &'static str> {
    let id = manifest.checked_id()?;
    if id != expected_execution_id {
        return Err("independent execution content pin mismatch");
    }
    if manifest
        .development_splits
        .iter()
        .any(|split| !allowed_development_splits.contains(&split.name))
    {
        return Err("development split not authorized by caller");
    }
    let required = std::iter::once(&manifest.protocol)
        .chain(manifest.inputs.iter())
        .chain(manifest.development_splits.iter())
        .collect::<Vec<_>>();
    let required_names = required
        .iter()
        .map(|input| &input.name)
        .collect::<BTreeSet<_>>();
    if required_names.len() != required.len() || source_bytes.len() != required.len() {
        return Err("execution source inventory mismatch");
    }
    let mut seen = BTreeSet::new();
    let mut total = payload.len();
    for (name, bytes) in source_bytes {
        total = total
            .checked_add(bytes.len())
            .ok_or("execution byte budget overflow")?;
        if total > MAX_PAYLOAD_BYTES || !seen.insert(name) {
            return Err("execution byte budget or source uniqueness violation");
        }
        let input = required
            .iter()
            .find(|input| input.name == *name)
            .ok_or("unexpected execution source")?;
        if sha256_bytes(bytes) != input.sha256 {
            return Err("execution protocol/input/split bytes mismatch");
        }
    }
    let entry = manifest
        .export
        .entry(entry_id)
        .ok_or("execution entry not found")?;
    if sha256_bytes(payload) != entry.payload_digest {
        return Err("execution output bytes mismatch");
    }
    let observation = Observation::from_payload_digest(
        entry.source_label.observation_kind(),
        format!("bench-execution://{}/{entry_id}", hex_id(&id)),
        entry.payload_digest,
        vec![],
    );
    let evidence = EvidenceClaim::from_observations(
        claim_id,
        &[&observation],
        entry.source_label.default_strength(),
    )
    .map_err(|_| "invalid execution evidence")?;
    Ok(BenchExecutionIngest {
        execution_id: id,
        source_label: entry.source_label,
        observation,
        evidence,
    })
}

fn hex_id(id: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in id {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BenchExportEntry, BenchKind, Claim, ClaimBody, ClaimStatus};

    fn fixture() -> (ClaimId, BenchExecutionManifest, Vec<(String, Vec<u8>)>) {
        let claim = Claim::new(ClaimBody {
            statement: "finite development observation".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let sources: Vec<(String, Vec<u8>)> = vec![
            ("protocol".into(), b"development-only protocol".to_vec()),
            ("input".into(), b"finite input bytes".to_vec()),
            (
                "development/panel".into(),
                b"frozen development membership".to_vec(),
            ),
        ];
        let identity = |index: usize| ExecutionInput {
            name: sources[index].0.clone(),
            sha256: sha256_bytes(&sources[index].1),
        };
        let export = BenchExportManifest::new(
            BenchKind::Riemann,
            "Memorithm/riemann_ndim_bench",
            "a".repeat(40),
            "run-one",
            vec![BenchExportEntry::from_payload(
                "out",
                BenchSourceLabel::Numerical,
                "finite comparison only",
                "output.json",
                b"recorded result",
            )],
        )
        .unwrap();
        let manifest = BenchExecutionManifest {
            schema_version: 1,
            export,
            protocol: identity(0),
            inputs: vec![identity(1)],
            development_splits: vec![identity(2)],
            ci_run_id: 123,
            ci_attempt: 1,
            ci_url: "https://github.com/Memorithm/riemann_ndim_bench/actions/runs/123".into(),
        };
        (claim.id, manifest, sources)
    }

    fn allowed() -> BTreeSet<String> {
        ["development/panel".into()].into_iter().collect()
    }

    #[test]
    fn ingest_preserves_numerical_scope_and_binds_every_identity() {
        let (claim, manifest, sources) = fixture();
        let id = manifest.checked_id().unwrap();
        let ingest = ingest_bench_execution(
            claim,
            &manifest,
            id,
            &allowed(),
            "out",
            b"recorded result",
            &sources,
        )
        .unwrap();
        assert_eq!(ingest.source_label, BenchSourceLabel::Numerical);
        assert_eq!(ingest.evidence.status, ClaimStatus::Observed);
        assert!(ingest.observation.check_id());
        assert!(ingest.evidence.check_id());
        assert!(ingest.observation.source_label.contains(&hex_id(&id)));
        assert!(crate::refuse_riemann_stub_proof_seal(ingest.source_label).is_err());
    }

    #[test]
    fn independent_pin_refuses_revision_protocol_split_output_and_run_substitution() {
        let (claim, manifest, sources) = fixture();
        let id = manifest.checked_id().unwrap();
        let mut variants = Vec::new();
        let mut changed = manifest.clone();
        changed.export.source_revision = "b".repeat(40);
        variants.push(changed);
        let mut changed = manifest.clone();
        changed.protocol.sha256[0] ^= 1;
        variants.push(changed);
        let mut changed = manifest.clone();
        changed.development_splits[0].sha256[0] ^= 1;
        variants.push(changed);
        let mut changed = manifest.clone();
        changed.inputs[0].sha256[0] ^= 1;
        variants.push(changed);
        let mut changed = manifest.clone();
        changed.export.entries[0].payload_digest[0] ^= 1;
        variants.push(changed);
        let mut changed = manifest.clone();
        changed.ci_attempt += 1;
        variants.push(changed);
        for changed in variants {
            assert_ne!(changed.checked_id().unwrap(), id);
            assert!(
                ingest_bench_execution(
                    claim,
                    &changed,
                    id,
                    &allowed(),
                    "out",
                    b"recorded result",
                    &sources
                )
                .is_err()
            );
        }
    }

    #[test]
    fn actual_source_and_output_bytes_are_required_and_verified() {
        let (claim, manifest, sources) = fixture();
        let id = manifest.checked_id().unwrap();
        assert!(
            ingest_bench_execution(
                claim,
                &manifest,
                id,
                &allowed(),
                "out",
                b"wrong result",
                &sources
            )
            .is_err()
        );
        for index in 0..sources.len() {
            let mut changed = sources.clone();
            changed[index].1[0] ^= 1;
            assert!(
                ingest_bench_execution(
                    claim,
                    &manifest,
                    id,
                    &allowed(),
                    "out",
                    b"recorded result",
                    &changed
                )
                .is_err()
            );
        }
        let missing = &sources[..2];
        assert!(
            ingest_bench_execution(
                claim,
                &manifest,
                id,
                &allowed(),
                "out",
                b"recorded result",
                missing
            )
            .is_err()
        );
        let mut duplicate = sources.clone();
        duplicate[2] = duplicate[1].clone();
        assert!(
            ingest_bench_execution(
                claim,
                &manifest,
                id,
                &allowed(),
                "out",
                b"recorded result",
                &duplicate
            )
            .is_err()
        );
    }

    #[test]
    fn caller_split_policy_and_development_only_boundary_fail_closed() {
        let (claim, mut manifest, sources) = fixture();
        let id = manifest.checked_id().unwrap();
        assert!(
            ingest_bench_execution(
                claim,
                &manifest,
                id,
                &BTreeSet::new(),
                "out",
                b"recorded result",
                &sources
            )
            .is_err()
        );
        manifest.development_splits[0].name = "test/panel".into();
        assert!(manifest.checked_id().is_err());
        manifest.development_splits[0].name = "final/panel".into();
        assert!(manifest.checked_id().is_err());
    }

    #[test]
    fn rejects_noncanonical_provenance_wrong_ci_and_oversize_before_hashing() {
        let (_, mut manifest, _) = fixture();
        manifest.inputs.push(manifest.inputs[0].clone());
        assert!(manifest.checked_id().is_err());
        let (_, mut manifest, _) = fixture();
        manifest.ci_url = "https://github.com/other/repository/actions/runs/123".into();
        assert!(manifest.checked_id().is_err());
        let (_, mut manifest, _) = fixture();
        manifest.export.source_revision = "a".repeat(MAX_TEXT + 1);
        assert!(manifest.checked_id().is_err());
        let (_, mut manifest, _) = fixture();
        manifest.export.entries = vec![manifest.export.entries[0].clone(); MAX_ITEMS + 1];
        assert!(manifest.checked_id().is_err());
    }

    #[test]
    fn unrecognized_wire_fields_are_not_silently_ignored() {
        let (_, manifest, _) = fixture();
        let mut json = serde_json::to_value(&manifest).unwrap();
        json["authenticated"] = serde_json::json!(true);
        assert!(serde_json::from_value::<BenchExecutionManifest>(json).is_err());
    }
}
