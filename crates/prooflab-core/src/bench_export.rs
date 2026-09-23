//! Revision-pinned external bench export manifests for PL-2.0.
//!
//! This module is the production-facing successor to the fixture-only Riemann/TDI
//! stub adapters. `ProofLab` does not fetch remote data here. Instead, a producing
//! bench exports a deterministic manifest that pins its repository, revision,
//! payload references, payload SHA-256 digests and epistemic labels.
//!
//! Importing such a manifest creates `Observation` / `EvidenceClaim` objects only.
//! Source labels remain provenance labels and never authorize PROVED.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    BenchSourceLabel, Canonical, CanonicalEncoder, ClaimId, ClaimStatus, EvidenceClaim,
    EvidenceError, Observation, sha256_bytes, sha256_canonical,
};

const BENCH_EXPORT_MANIFEST_DOMAIN: &[u8] = b"prooflab:bench-export-manifest:v1\0";

/// External Memorithm research bench that produced an export manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchKind {
    Riemann,
    Tdi,
}

impl BenchKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Riemann => "riemann",
            Self::Tdi => "tdi",
        }
    }
}

impl Canonical for BenchKind {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str(self.as_str());
    }
}

impl Canonical for BenchSourceLabel {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str(self.as_str());
    }
}

/// One externally produced evidence item.
///
/// The payload itself may live in another repository or artifact store.
/// `payload_ref` says where the producer recorded it and `payload_digest` pins
/// its exact bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchExportEntry {
    pub entry_id: String,
    pub source_label: BenchSourceLabel,
    pub statement: String,
    pub payload_ref: String,
    pub payload_digest: [u8; 32],
}

impl BenchExportEntry {
    /// Construct an entry from locally available payload bytes.
    #[must_use]
    pub fn from_payload(
        entry_id: impl Into<String>,
        source_label: BenchSourceLabel,
        statement: impl Into<String>,
        payload_ref: impl Into<String>,
        payload: &[u8],
    ) -> Self {
        Self {
            entry_id: entry_id.into(),
            source_label,
            statement: statement.into(),
            payload_ref: payload_ref.into(),
            payload_digest: sha256_bytes(payload),
        }
    }

    /// Construct an entry when the producer already supplied a SHA-256 digest.
    #[must_use]
    pub fn from_digest(
        entry_id: impl Into<String>,
        source_label: BenchSourceLabel,
        statement: impl Into<String>,
        payload_ref: impl Into<String>,
        payload_digest: [u8; 32],
    ) -> Self {
        Self {
            entry_id: entry_id.into(),
            source_label,
            statement: statement.into(),
            payload_ref: payload_ref.into(),
            payload_digest,
        }
    }
}

impl Canonical for BenchExportEntry {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.bench-export-entry/v1");
        encoder.value(&self.entry_id);
        encoder.value(&self.source_label);
        encoder.value(&self.statement);
        encoder.value(&self.payload_ref);
        encoder.value(&self.payload_digest);
    }
}

/// Content identity of a canonical external bench export manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BenchExportManifestId(pub [u8; 32]);

impl Canonical for BenchExportManifestId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

/// Deterministic offline export from one exact bench revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchExportManifest {
    pub bench: BenchKind,
    pub source_repository: String,
    pub source_revision: String,
    pub export_id: String,
    pub entries: Vec<BenchExportEntry>,
}

impl BenchExportManifest {
    /// Construct and canonicalize one manifest.
    ///
    /// Entries are sorted by `entry_id`; duplicate IDs and empty provenance
    /// fields fail closed.
    ///
    /// # Errors
    ///
    /// Returns [`BenchExportError`] when mandatory manifest/entry provenance is
    /// empty or when entry identifiers are duplicated.
    pub fn new(
        bench: BenchKind,
        source_repository: impl Into<String>,
        source_revision: impl Into<String>,
        export_id: impl Into<String>,
        mut entries: Vec<BenchExportEntry>,
    ) -> Result<Self, BenchExportError> {
        entries.sort_by(|left, right| left.entry_id.cmp(&right.entry_id));
        let manifest = Self {
            bench,
            source_repository: source_repository.into(),
            source_revision: source_revision.into(),
            export_id: export_id.into(),
            entries,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate canonical ordering and all mandatory provenance fields.
    ///
    /// # Errors
    ///
    /// Returns [`BenchExportError`] for empty fields, duplicate identifiers or
    /// deserialized entry order that is not canonical.
    pub fn validate(&self) -> Result<(), BenchExportError> {
        if self.source_repository.trim().is_empty() {
            return Err(BenchExportError::EmptySourceRepository);
        }
        if self.source_revision.trim().is_empty() {
            return Err(BenchExportError::EmptySourceRevision);
        }
        if !is_canonical_git_revision(&self.source_revision) {
            return Err(BenchExportError::InvalidSourceRevision(
                self.source_revision.clone(),
            ));
        }
        if self.export_id.trim().is_empty() {
            return Err(BenchExportError::EmptyExportId);
        }
        if self.entries.is_empty() {
            return Err(BenchExportError::EmptyEntries);
        }

        let mut previous: Option<&str> = None;
        for entry in &self.entries {
            validate_entry(entry)?;
            if let Some(previous) = previous {
                match previous.cmp(entry.entry_id.as_str()) {
                    core::cmp::Ordering::Less => {}
                    core::cmp::Ordering::Equal => {
                        return Err(BenchExportError::DuplicateEntryId(entry.entry_id.clone()));
                    }
                    core::cmp::Ordering::Greater => {
                        return Err(BenchExportError::NonCanonicalEntryOrder);
                    }
                }
            }
            previous = Some(entry.entry_id.as_str());
        }
        Ok(())
    }

    /// Content identity of this exact canonical export.
    #[must_use]
    pub fn id(&self) -> BenchExportManifestId {
        BenchExportManifestId(sha256_canonical(BENCH_EXPORT_MANIFEST_DOMAIN, self))
    }

    /// Find one entry by its canonical ID.
    #[must_use]
    pub fn entry(&self, entry_id: &str) -> Option<&BenchExportEntry> {
        self.entries
            .binary_search_by(|entry| entry.entry_id.as_str().cmp(entry_id))
            .ok()
            .map(|index| &self.entries[index])
    }
}

impl Canonical for BenchExportManifest {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.bench-export-manifest/v1");
        encoder.value(&self.bench);
        encoder.value(&self.source_repository);
        encoder.value(&self.source_revision);
        encoder.value(&self.export_id);
        encoder.seq(&self.entries);
    }
}

fn is_canonical_git_revision(revision: &str) -> bool {
    matches!(revision.len(), 40 | 64)
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_entry(entry: &BenchExportEntry) -> Result<(), BenchExportError> {
    if entry.entry_id.trim().is_empty() {
        return Err(BenchExportError::EmptyEntryId);
    }
    if entry.statement.trim().is_empty() {
        return Err(BenchExportError::EmptyStatement);
    }
    if entry.payload_ref.trim().is_empty() {
        return Err(BenchExportError::EmptyPayloadRef);
    }
    Ok(())
}

/// Provenance-preserving result of importing one entry from a bench export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchExportIngest {
    pub manifest_id: BenchExportManifestId,
    pub bench: BenchKind,
    pub source_repository: String,
    pub source_revision: String,
    pub export_id: String,
    pub entry_id: String,
    pub source_label: BenchSourceLabel,
    pub statement: String,
    pub payload_ref: String,
    pub payload_digest: [u8; 32],
    pub observation: Observation,
    pub evidence: EvidenceClaim,
}

/// Ingest one entry from a validated, revision-pinned external manifest.
///
/// The caller explicitly binds the entry to `claim_id`. `ProofLab` preserves that
/// mapping in the resulting `EvidenceClaim` but does not infer proof status.
///
/// # Errors
///
/// Fails closed on malformed/canonicalization-invalid manifests, missing entries,
/// or evidence construction failures.
pub fn ingest_bench_export(
    claim_id: ClaimId,
    manifest: &BenchExportManifest,
    entry_id: &str,
) -> Result<BenchExportIngest, BenchExportError> {
    manifest.validate()?;
    let entry = manifest
        .entry(entry_id)
        .ok_or_else(|| BenchExportError::EntryNotFound(entry_id.to_owned()))?;
    let manifest_id = manifest.id();
    let source_label = format!(
        "bench-export://{}/{}/{}/{}",
        manifest.bench.as_str(),
        hex32(&manifest_id.0),
        entry.entry_id,
        entry.source_label.as_str()
    );
    let observation = Observation::from_payload_digest(
        entry.source_label.observation_kind(),
        source_label,
        entry.payload_digest,
        vec![],
    );
    let evidence = EvidenceClaim::from_observations(
        claim_id,
        &[&observation],
        entry.source_label.default_strength(),
    )?;

    debug_assert_eq!(evidence.status, ClaimStatus::Observed);
    debug_assert_ne!(evidence.status, ClaimStatus::Proved);

    Ok(BenchExportIngest {
        manifest_id,
        bench: manifest.bench,
        source_repository: manifest.source_repository.clone(),
        source_revision: manifest.source_revision.clone(),
        export_id: manifest.export_id.clone(),
        entry_id: entry.entry_id.clone(),
        source_label: entry.source_label,
        statement: entry.statement.clone(),
        payload_ref: entry.payload_ref.clone(),
        payload_digest: entry.payload_digest,
        observation,
        evidence,
    })
}

/// Verify locally available payload bytes against one exported digest.
///
/// This helper performs no I/O or network access. Callers decide how the bytes
/// were obtained and can require this check before ingesting or replaying data.
///
/// # Errors
///
/// Returns [`BenchExportError::PayloadDigestMismatch`] when `payload` does not
/// match the SHA-256 digest pinned by the exported entry.
pub fn verify_bench_export_payload(
    entry: &BenchExportEntry,
    payload: &[u8],
) -> Result<(), BenchExportError> {
    if sha256_bytes(payload) == entry.payload_digest {
        Ok(())
    } else {
        Err(BenchExportError::PayloadDigestMismatch {
            entry_id: entry.entry_id.clone(),
        })
    }
}

/// Validation/import failures for revision-pinned bench exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchExportError {
    EmptySourceRepository,
    EmptySourceRevision,
    InvalidSourceRevision(String),
    EmptyExportId,
    EmptyEntries,
    EmptyEntryId,
    EmptyStatement,
    EmptyPayloadRef,
    DuplicateEntryId(String),
    NonCanonicalEntryOrder,
    EntryNotFound(String),
    PayloadDigestMismatch { entry_id: String },
    Evidence(EvidenceError),
}

impl fmt::Display for BenchExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceRepository => {
                formatter.write_str("bench export source_repository is empty")
            }
            Self::EmptySourceRevision => {
                formatter.write_str("bench export source_revision is empty")
            }
            Self::InvalidSourceRevision(revision) => write!(
                formatter,
                "bench export source_revision must be a canonical 40- or 64-character lowercase Git object id, found {revision:?}"
            ),
            Self::EmptyExportId => formatter.write_str("bench export export_id is empty"),
            Self::EmptyEntries => formatter.write_str("bench export contains no entries"),
            Self::EmptyEntryId => formatter.write_str("bench export entry_id is empty"),
            Self::EmptyStatement => formatter.write_str("bench export statement is empty"),
            Self::EmptyPayloadRef => formatter.write_str("bench export payload_ref is empty"),
            Self::DuplicateEntryId(entry_id) => {
                write!(formatter, "duplicate bench export entry_id: {entry_id}")
            }
            Self::NonCanonicalEntryOrder => {
                formatter.write_str("bench export entries are not in canonical entry_id order")
            }
            Self::EntryNotFound(entry_id) => {
                write!(formatter, "bench export entry not found: {entry_id}")
            }
            Self::PayloadDigestMismatch { entry_id } => {
                write!(
                    formatter,
                    "bench export payload digest mismatch for entry {entry_id}"
                )
            }
            Self::Evidence(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for BenchExportError {}

impl From<EvidenceError> for BenchExportError {
    fn from(value: EvidenceError) -> Self {
        Self::Evidence(value)
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Claim, ClaimBody, EvidenceStrength};

    fn claim() -> Claim {
        Claim::new(ClaimBody {
            statement: "revision-pinned external bench evidence".into(),
            assumptions: vec![],
            parents: vec![],
        })
    }

    fn entry(id: &str, label: BenchSourceLabel, payload: &[u8]) -> BenchExportEntry {
        BenchExportEntry::from_payload(
            id,
            label,
            "exported regularity",
            format!("artifacts/{id}.json"),
            payload,
        )
    }

    fn manifest(entries: Vec<BenchExportEntry>) -> BenchExportManifest {
        BenchExportManifest::new(
            BenchKind::Tdi,
            "Memorithm/TDI",
            "0123456789abcdef0123456789abcdef01234567",
            "campaign-2026-09-23",
            entries,
        )
        .unwrap()
    }

    #[test]
    fn manifest_identity_is_order_independent_after_canonicalization() {
        let a = entry("a", BenchSourceLabel::Numerical, b"a");
        let b = entry("b", BenchSourceLabel::Exact, b"b");
        let left = manifest(vec![a.clone(), b.clone()]);
        let right = manifest(vec![b, a]);
        assert_eq!(left.entries, right.entries);
        assert_eq!(left.id(), right.id());
    }

    #[test]
    fn revision_and_payload_digest_change_manifest_identity() {
        let base = manifest(vec![entry("a", BenchSourceLabel::Numerical, b"a")]);
        let changed_revision = BenchExportManifest::new(
            BenchKind::Tdi,
            "Memorithm/TDI",
            "fedcba9876543210fedcba9876543210fedcba98",
            "campaign-2026-09-23",
            base.entries.clone(),
        )
        .unwrap();
        let changed_payload = manifest(vec![entry("a", BenchSourceLabel::Numerical, b"other")]);
        assert_ne!(base.id(), changed_revision.id());
        assert_ne!(base.id(), changed_payload.id());
    }

    #[test]
    fn ingest_preserves_revision_label_and_exact_payload_digest() {
        let payload = b"real-exported-measurement-bytes";
        let manifest = manifest(vec![entry("run-17", BenchSourceLabel::Numerical, payload)]);
        let claim = claim();
        let ingest = ingest_bench_export(claim.id, &manifest, "run-17").unwrap();

        assert_eq!(ingest.manifest_id, manifest.id());
        assert_eq!(
            ingest.source_revision,
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(ingest.source_repository, "Memorithm/TDI");
        assert_eq!(ingest.source_label, BenchSourceLabel::Numerical);
        assert_eq!(ingest.payload_digest, sha256_bytes(payload));
        assert_eq!(ingest.observation.payload_digest, sha256_bytes(payload));
        assert!(
            ingest
                .observation
                .source_label
                .starts_with("bench-export://tdi/")
        );
        assert_eq!(ingest.evidence.strength, EvidenceStrength::Suggestive);
        assert_eq!(ingest.evidence.status, ClaimStatus::Observed);
        assert_ne!(ingest.evidence.status, ClaimStatus::Proved);
        assert!(ingest.observation.check_id());
        assert!(ingest.evidence.check_id());
    }

    #[test]
    fn payload_bytes_can_be_verified_without_network_access() {
        let entry = entry("payload", BenchSourceLabel::Exact, b"payload");
        assert_eq!(verify_bench_export_payload(&entry, b"payload"), Ok(()));
        assert_eq!(
            verify_bench_export_payload(&entry, b"tampered"),
            Err(BenchExportError::PayloadDigestMismatch {
                entry_id: "payload".into()
            })
        );
    }

    #[test]
    fn malformed_or_ambiguous_manifests_fail_closed() {
        let duplicate = BenchExportManifest::new(
            BenchKind::Riemann,
            "Memorithm/RiemannBench",
            "0123456789abcdef0123456789abcdef01234567",
            "export",
            vec![
                entry("same", BenchSourceLabel::Numerical, b"a"),
                entry("same", BenchSourceLabel::Exact, b"b"),
            ],
        );
        assert_eq!(
            duplicate,
            Err(BenchExportError::DuplicateEntryId("same".into()))
        );

        let empty = BenchExportManifest::new(
            BenchKind::Tdi,
            "",
            "rev",
            "export",
            vec![entry("a", BenchSourceLabel::Numerical, b"a")],
        );
        assert_eq!(empty, Err(BenchExportError::EmptySourceRepository));
    }

    #[test]
    fn mutable_or_noncanonical_source_revisions_are_rejected() {
        let entry = entry("a", BenchSourceLabel::Numerical, b"a");
        for revision in [
            "main",
            "0123456789abcdef",
            "ABCDEF0123456789ABCDEF0123456789ABCDEF01",
        ] {
            assert!(matches!(
                BenchExportManifest::new(
                    BenchKind::Tdi,
                    "Memorithm/TDI",
                    revision,
                    "export",
                    vec![entry.clone()],
                ),
                Err(BenchExportError::InvalidSourceRevision(_))
            ));
        }
    }

    #[test]
    fn deserialized_noncanonical_entry_order_is_rejected() {
        let mut manifest = manifest(vec![
            entry("a", BenchSourceLabel::Numerical, b"a"),
            entry("b", BenchSourceLabel::Exact, b"b"),
        ]);
        manifest.entries.swap(0, 1);
        assert_eq!(
            manifest.validate(),
            Err(BenchExportError::NonCanonicalEntryOrder)
        );
        assert!(matches!(
            ingest_bench_export(claim().id, &manifest, "a"),
            Err(BenchExportError::NonCanonicalEntryOrder)
        ));
    }

    #[test]
    fn missing_entry_fails_without_creating_evidence() {
        let manifest = manifest(vec![entry("a", BenchSourceLabel::Numerical, b"a")]);
        assert_eq!(
            ingest_bench_export(claim().id, &manifest, "missing"),
            Err(BenchExportError::EntryNotFound("missing".into()))
        );
    }
}
