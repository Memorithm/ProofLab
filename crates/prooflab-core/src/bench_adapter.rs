//! PL-2.0 label-preserving stub adapter for one external bench (Riemann).
//!
//! Ingests fixture-style evidence manifests into [`Observation`] /
//! [`EvidenceClaim`] only. Epistemic source labels from
//! `riemann_ndim_bench` (and the PL-C3 vocabulary) are preserved exactly:
//! they are never silently upgraded (e.g. `numerical` → `exact`, or any
//! label → `PROVED`).
//!
//! Non-claims: this is not live TDI/Riemann network ingest, does not
//! establish mathematical novelty, and never seals a [`crate::ProofArtifact`].
//! Lean remains the sole `PROVED` authority. `empirical ≠ proof`.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::evidence::{
    EvidenceClaim, EvidenceError, EvidenceStrength, Observation, ObservationKind,
    refuse_empirical_proof_seal,
};
use crate::{ClaimId, ClaimStatus};

/// Epistemic source labels admitted by the Riemann stub adapter.
///
/// Canonical wire tags match the PL-C3 / `riemann_ndim_bench` vocabulary.
/// Variants are provenance tags only; none confers [`ClaimStatus::Proved`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchSourceLabel {
    /// Finite algebraic identity in the bench (still not Lean-proved).
    Exact,
    /// Finite numerical observation or regression.
    Numerical,
    /// Conjectural implication (source or project).
    Conjecture,
    /// Derived asymptotic without a theorem-grade remainder.
    FormalAsymptotic,
}

impl BenchSourceLabel {
    /// Canonical lowercase tag preserved on import.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Numerical => "numerical",
            Self::Conjecture => "conjecture",
            Self::FormalAsymptotic => "formal_asymptotic",
        }
    }

    /// Parse a label string without upgrading aliases to stronger classes.
    ///
    /// Accepts the canonical tags and a few documented synonyms from
    /// `riemann_ndim_bench` (`numerical_evidence`, `source_conjecture`,
    /// `formal asymptotic`). Rejects proof-like tokens (`proved`, `theorem`, …).
    ///
    /// # Errors
    ///
    /// Returns [`BenchAdapterError::UnknownLabel`] or
    /// [`BenchAdapterError::ForbiddenProofLabel`] when the token is not an
    /// admitted epistemic class.
    pub fn parse(raw: &str) -> Result<Self, BenchAdapterError> {
        let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
        let normalized = normalized.replace(' ', "_");
        match normalized.as_str() {
            "exact" => Ok(Self::Exact),
            "numerical" | "numerical_evidence" => Ok(Self::Numerical),
            "conjecture" | "source_conjecture" => Ok(Self::Conjecture),
            "formal_asymptotic" => Ok(Self::FormalAsymptotic),
            "proved" | "proof" | "theorem" | "source_theorem" | "lean_proved" | "verified" => {
                Err(BenchAdapterError::ForbiddenProofLabel(raw.into()))
            }
            other => Err(BenchAdapterError::UnknownLabel(other.into())),
        }
    }

    /// Map to an [`ObservationKind`] without strengthening the epistemic class.
    ///
    /// Numerical stays numerical; exact / formal-asymptotic stay symbolic
    /// experiment (not kernel acceptance); conjecture stays annotation.
    #[must_use]
    pub const fn observation_kind(self) -> ObservationKind {
        match self {
            Self::Numerical => ObservationKind::Numerical,
            Self::Exact | Self::FormalAsymptotic => ObservationKind::SymbolicExperiment,
            Self::Conjecture => ObservationKind::ManualAnnotation,
        }
    }

    /// Conservative evidence strength ceiling for this label.
    ///
    /// Never returns a strength that could be confused with proof; numerical
    /// and asymptotic evidence stay [`EvidenceStrength::Suggestive`].
    #[must_use]
    pub const fn default_strength(self) -> EvidenceStrength {
        match self {
            Self::Exact => EvidenceStrength::Corroborated,
            Self::Numerical | Self::Conjecture | Self::FormalAsymptotic => {
                EvidenceStrength::Suggestive
            }
        }
    }
}

impl fmt::Display for BenchSourceLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One fixture row from a Riemann (or PL-C3) evidence manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiemannStubEntry {
    /// Stable entry id within the stub manifest (not a content address).
    pub entry_id: String,
    /// Epistemic label; preserved exactly through ingest.
    pub source_label: BenchSourceLabel,
    /// Human-readable statement sketch motivating a claim (not a proof).
    pub statement: String,
    /// Opaque payload bytes recorded via digest on the observation.
    pub payload: String,
}

/// Result of stub-ingest: observation + evidence only (never a proof).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiemannStubIngest {
    /// Preserved epistemic label from the entry.
    pub source_label: BenchSourceLabel,
    pub observation: Observation,
    pub evidence: EvidenceClaim,
}

/// Failures while adapting external bench labels into typed evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchAdapterError {
    EmptyEntryId,
    EmptyPayload,
    EmptyStatement,
    UnknownLabel(String),
    ForbiddenProofLabel(String),
    LabelUpgrade {
        from: BenchSourceLabel,
        to: BenchSourceLabel,
    },
    Evidence(EvidenceError),
}

impl fmt::Display for BenchAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyEntryId => write!(formatter, "riemann stub entry_id must be non-empty"),
            Self::EmptyPayload => write!(formatter, "riemann stub payload must be non-empty"),
            Self::EmptyStatement => write!(formatter, "riemann stub statement must be non-empty"),
            Self::UnknownLabel(label) => {
                write!(formatter, "unknown bench source label: {label}")
            }
            Self::ForbiddenProofLabel(label) => {
                write!(
                    formatter,
                    "bench adapter refuses proof-like label on import: {label}"
                )
            }
            Self::LabelUpgrade { from, to } => {
                write!(
                    formatter,
                    "refusing silent label upgrade on import: {from} -> {to}"
                )
            }
            Self::Evidence(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for BenchAdapterError {}

impl From<EvidenceError> for BenchAdapterError {
    fn from(value: EvidenceError) -> Self {
        Self::Evidence(value)
    }
}

/// Refuse an attempted silent upgrade between epistemic labels.
///
/// # Errors
///
/// Returns [`BenchAdapterError::LabelUpgrade`] when `observed != original`.
pub fn refuse_label_upgrade(
    original: BenchSourceLabel,
    observed: BenchSourceLabel,
) -> Result<(), BenchAdapterError> {
    if original == observed {
        Ok(())
    } else {
        Err(BenchAdapterError::LabelUpgrade {
            from: original,
            to: observed,
        })
    }
}

/// Ingest one Riemann stub manifest entry into [`Observation`] / [`EvidenceClaim`].
///
/// The entry's [`BenchSourceLabel`] is embedded in `Observation::source_label`
/// and returned unchanged on [`RiemannStubIngest`]. Status is always
/// [`ClaimStatus::Observed`]. This path cannot construct a proof artifact.
///
/// # Errors
///
/// Fails on empty fields or evidence integrity errors.
pub fn ingest_riemann_stub(
    claim_id: ClaimId,
    entry: &RiemannStubEntry,
) -> Result<RiemannStubIngest, BenchAdapterError> {
    if entry.entry_id.trim().is_empty() {
        return Err(BenchAdapterError::EmptyEntryId);
    }
    if entry.payload.is_empty() {
        return Err(BenchAdapterError::EmptyPayload);
    }
    if entry.statement.trim().is_empty() {
        return Err(BenchAdapterError::EmptyStatement);
    }

    let label = entry.source_label;
    // Identity check: re-parsing the canonical tag must not change the class.
    let reparsed = BenchSourceLabel::parse(label.as_str())?;
    refuse_label_upgrade(label, reparsed)?;

    let source_label = format!(
        "riemann-stub://{}/{}",
        entry.entry_id.trim(),
        label.as_str()
    );
    let observation = Observation::new(
        label.observation_kind(),
        source_label,
        entry.payload.as_bytes(),
        vec![],
    );
    let evidence =
        EvidenceClaim::from_observations(claim_id, &[&observation], label.default_strength())?;

    debug_assert_eq!(evidence.status, ClaimStatus::Observed);
    debug_assert_ne!(evidence.status, ClaimStatus::Proved);
    debug_assert_eq!(observation.implied_status(), ClaimStatus::Observed);

    Ok(RiemannStubIngest {
        source_label: label,
        observation,
        evidence,
    })
}

/// Deny helper: stub-adapted empirical evidence cannot seal proofs.
///
/// # Errors
///
/// Always returns [`EvidenceError::EmpiricalCannotSealProof`].
pub fn refuse_riemann_stub_proof_seal(
    label: BenchSourceLabel,
) -> Result<crate::ProofArtifact, EvidenceError> {
    // ObservationKind deny is sufficient; strength deny is covered by unit tests.
    refuse_empirical_proof_seal(label.observation_kind())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Claim, ClaimBody};

    fn claim() -> Claim {
        Claim::new(ClaimBody {
            statement: "finite compression diagnostic (stub)".into(),
            assumptions: vec!["finite block size".into()],
            parents: vec![],
        })
    }

    fn entry(label: BenchSourceLabel, id: &str) -> RiemannStubEntry {
        RiemannStubEntry {
            entry_id: id.into(),
            source_label: label,
            statement: "stub regularity for typed ingest".into(),
            payload: format!("payload-{id}"),
        }
    }

    #[test]
    fn numerical_label_is_preserved_and_never_proved() {
        let claim = claim();
        let ingest = ingest_riemann_stub(claim.id, &entry(BenchSourceLabel::Numerical, "num-1"))
            .expect("ingest");
        assert_eq!(ingest.source_label, BenchSourceLabel::Numerical);
        assert_eq!(ingest.source_label.as_str(), "numerical");
        assert!(ingest.observation.source_label.ends_with("/numerical"));
        assert_eq!(ingest.observation.kind, ObservationKind::Numerical);
        assert_eq!(ingest.evidence.status, ClaimStatus::Observed);
        assert_ne!(ingest.evidence.status, ClaimStatus::Proved);
        assert_eq!(ingest.evidence.strength, EvidenceStrength::Suggestive);
        assert!(ingest.observation.check_id());
        assert!(ingest.evidence.check_id());
    }

    #[test]
    fn exact_and_formal_asymptotic_are_not_upgraded_to_each_other() {
        let claim = claim();
        let exact = ingest_riemann_stub(claim.id, &entry(BenchSourceLabel::Exact, "ex-1")).unwrap();
        let asym =
            ingest_riemann_stub(claim.id, &entry(BenchSourceLabel::FormalAsymptotic, "fa-1"))
                .unwrap();
        assert_eq!(exact.source_label, BenchSourceLabel::Exact);
        assert_eq!(asym.source_label, BenchSourceLabel::FormalAsymptotic);
        assert_ne!(exact.source_label, asym.source_label);
        assert!(
            refuse_label_upgrade(BenchSourceLabel::Numerical, BenchSourceLabel::Exact).is_err()
        );
        assert!(
            refuse_label_upgrade(BenchSourceLabel::FormalAsymptotic, BenchSourceLabel::Exact)
                .is_err()
        );
        assert!(
            refuse_label_upgrade(BenchSourceLabel::Conjecture, BenchSourceLabel::Exact).is_err()
        );
    }

    #[test]
    fn parse_rejects_proof_like_labels_and_unknown_upgrades() {
        assert_eq!(
            BenchSourceLabel::parse("proved"),
            Err(BenchAdapterError::ForbiddenProofLabel("proved".into()))
        );
        assert_eq!(
            BenchSourceLabel::parse("source_theorem"),
            Err(BenchAdapterError::ForbiddenProofLabel(
                "source_theorem".into()
            ))
        );
        assert_eq!(
            BenchSourceLabel::parse("numerical_evidence"),
            Ok(BenchSourceLabel::Numerical)
        );
        assert_eq!(
            BenchSourceLabel::parse("formal asymptotic"),
            Ok(BenchSourceLabel::FormalAsymptotic)
        );
        assert!(matches!(
            BenchSourceLabel::parse("almost_proved"),
            Err(BenchAdapterError::UnknownLabel(_))
        ));
    }

    #[test]
    fn stub_ingest_cannot_seal_proof() {
        for label in [
            BenchSourceLabel::Exact,
            BenchSourceLabel::Numerical,
            BenchSourceLabel::Conjecture,
            BenchSourceLabel::FormalAsymptotic,
        ] {
            assert!(
                refuse_riemann_stub_proof_seal(label).is_err(),
                "label {label} must not seal proof"
            );
        }
    }

    #[test]
    fn serde_round_trip_preserves_labels_without_status_upgrade() {
        let claim = claim();
        let ingest =
            ingest_riemann_stub(claim.id, &entry(BenchSourceLabel::Conjecture, "cj-1")).unwrap();
        let bytes = serde_json::to_vec(&ingest).expect("serialize");
        let restored: RiemannStubIngest = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(restored.source_label, BenchSourceLabel::Conjecture);
        assert_eq!(restored.evidence.status, ClaimStatus::Observed);
        assert!(restored.evidence.check_id());

        let mut forged = serde_json::to_value(&ingest).unwrap();
        forged["source_label"] = serde_json::json!("exact");
        forged["evidence"]["status"] = serde_json::json!("Proved");
        let tampered: RiemannStubIngest = serde_json::from_value(forged).unwrap();
        // Label field can be rewritten in JSON, but evidence integrity fails
        // when status is upgraded, and refuse_label_upgrade catches the swap.
        assert!(!tampered.evidence.check_id());
        assert_eq!(
            refuse_label_upgrade(BenchSourceLabel::Conjecture, tampered.source_label),
            Err(BenchAdapterError::LabelUpgrade {
                from: BenchSourceLabel::Conjecture,
                to: BenchSourceLabel::Exact,
            })
        );
    }

    #[test]
    fn fixture_manifest_json_ingests_label_preserving() {
        let manifest = r#"
        [
          {
            "entry_id": "phase3-boundary-t0",
            "source_label": "numerical",
            "statement": "prolate boundary contribution t(0) regression",
            "payload": "t0=fixture"
          },
          {
            "entry_id": "phase4-soft-edge",
            "source_label": "formal_asymptotic",
            "statement": "soft-edge fixed-rank scaling (no theorem remainder)",
            "payload": "soft-edge-fixture"
          },
          {
            "entry_id": "semilocal-sufficiency",
            "source_label": "conjecture",
            "statement": "bounded-support semilocal sufficiency (source conjecture)",
            "payload": "conjecture-fixture"
          }
        ]
        "#;
        let entries: Vec<RiemannStubEntry> = serde_json::from_str(manifest).unwrap();
        assert_eq!(entries.len(), 3);
        let claim = claim();
        let labels: Vec<_> = entries
            .iter()
            .map(|entry| {
                let ingest = ingest_riemann_stub(claim.id, entry).unwrap();
                assert_ne!(ingest.evidence.status, ClaimStatus::Proved);
                ingest.source_label
            })
            .collect();
        assert_eq!(
            labels,
            [
                BenchSourceLabel::Numerical,
                BenchSourceLabel::FormalAsymptotic,
                BenchSourceLabel::Conjecture,
            ]
        );
    }
}
