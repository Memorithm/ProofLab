//! Bridge PL-1.1 falsification records into the PL-2.0 typed evidence chain.
//!
//! A validated [`FalsificationRecord`] / [`CounterexampleWitness`] pair becomes
//! an [`Observation`] + [`EvidenceClaim`] so cheap falsification participates in
//! the same typed provenance chain as stub bench ingest. Scientific status for
//! the claim remains [`ClaimStatus::Falsified`] on the record; the evidence
//! object stays [`ClaimStatus::Observed`] (a counterexample observation was
//! recorded). This path never seals [`ClaimStatus::Proved`] and refuses
//! conjecture promotion for falsified records.
//!
//! Non-claims: wiring falsification into typed evidence does not enlarge the
//! falsifier, does not authorize proof search, and does not claim completeness.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::evidence::{
    EvidenceClaim, EvidenceError, EvidenceStrength, Observation, ObservationKind,
    refuse_empirical_proof_seal,
};
use crate::falsify::{
    CounterexampleWitness, CounterexampleWitnessId, FalsificationRecord, FalsificationRecordId,
    FalsifyError,
};
use crate::{Claim, ClaimStatus, ProofArtifact};

/// Result of adapting a PL-1.1 falsification into typed evidence (never a proof).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FalsifyEvidenceIngest {
    /// Always [`ClaimStatus::Falsified`] — copied from the validated record.
    pub scientific_status: ClaimStatus,
    pub observation: Observation,
    /// Evidence that a counterexample was observed (`Observed` only; not `Proved`).
    pub evidence: EvidenceClaim,
    pub falsification_record_id: FalsificationRecordId,
    pub witness_id: CounterexampleWitnessId,
}

/// Failures while bridging falsification records into typed evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalsifyEvidenceError {
    RecordIntegrity,
    WitnessIntegrity,
    ClaimMismatch,
    WitnessMismatch,
    NotFalsified,
    Falsify(FalsifyError),
    Evidence(EvidenceError),
    /// Falsified claims must not be promoted to conjecture candidates.
    ConjectureRefused,
}

impl fmt::Display for FalsifyEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordIntegrity => write!(formatter, "falsification record id mismatch"),
            Self::WitnessIntegrity => write!(formatter, "counterexample witness failed validation"),
            Self::ClaimMismatch => {
                write!(
                    formatter,
                    "falsification record claim_id does not match claim"
                )
            }
            Self::WitnessMismatch => {
                write!(
                    formatter,
                    "falsification record witness_id does not match witness"
                )
            }
            Self::NotFalsified => {
                write!(formatter, "falsification record status is not Falsified")
            }
            Self::Falsify(error) => write!(formatter, "{error}"),
            Self::Evidence(error) => write!(formatter, "{error}"),
            Self::ConjectureRefused => write!(
                formatter,
                "refusing conjecture promotion for a falsified claim"
            ),
        }
    }
}

impl std::error::Error for FalsifyEvidenceError {}

impl From<FalsifyError> for FalsifyEvidenceError {
    fn from(value: FalsifyError) -> Self {
        Self::Falsify(value)
    }
}

impl From<EvidenceError> for FalsifyEvidenceError {
    fn from(value: EvidenceError) -> Self {
        Self::Evidence(value)
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use core::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Ingest a validated PL-1.1 falsification into [`Observation`] / [`EvidenceClaim`].
///
/// The observation uses [`ObservationKind::Counterexample`] with a
/// `falsify://pl-1.1/...` provenance URI. Evidence strength is
/// [`EvidenceStrength::Strong`] (concrete finite witness) but status remains
/// [`ClaimStatus::Observed`]. The record's [`ClaimStatus::Falsified`] is
/// preserved on [`FalsifyEvidenceIngest::scientific_status`]. This never
/// constructs a [`ProofArtifact`].
///
/// # Errors
///
/// Fails closed on integrity / identity mismatches or when the record is not
/// [`ClaimStatus::Falsified`].
pub fn evidence_from_falsification(
    claim: &Claim,
    witness: &CounterexampleWitness,
    record: &FalsificationRecord,
) -> Result<FalsifyEvidenceIngest, FalsifyEvidenceError> {
    if !record.check_id() {
        return Err(FalsifyEvidenceError::RecordIntegrity);
    }
    if record.status != ClaimStatus::Falsified {
        return Err(FalsifyEvidenceError::NotFalsified);
    }
    if record.claim_id != claim.id {
        return Err(FalsifyEvidenceError::ClaimMismatch);
    }
    if record.witness_id != witness.id {
        return Err(FalsifyEvidenceError::WitnessMismatch);
    }
    witness.validate()?;

    let source_label = format!("falsify://pl-1.1/{}", hex32(&record.id.0));
    let mut payload = Vec::with_capacity(64);
    payload.extend_from_slice(&record.id.0);
    payload.extend_from_slice(&witness.id.0);

    let observation = Observation::new(
        ObservationKind::Counterexample,
        source_label,
        &payload,
        vec![],
    );
    let evidence =
        EvidenceClaim::from_observations(claim.id, &[&observation], EvidenceStrength::Strong)?;

    debug_assert_eq!(evidence.status, ClaimStatus::Observed);
    debug_assert_eq!(record.status, ClaimStatus::Falsified);
    debug_assert_ne!(evidence.status, ClaimStatus::Proved);
    debug_assert_ne!(record.status, ClaimStatus::Proved);

    Ok(FalsifyEvidenceIngest {
        scientific_status: ClaimStatus::Falsified,
        observation,
        evidence,
        falsification_record_id: record.id,
        witness_id: witness.id,
    })
}

/// Explicit deny path: falsify-bridged evidence cannot seal a proof artifact.
///
/// # Errors
///
/// Always returns [`EvidenceError::EmpiricalCannotSealProof`].
pub fn refuse_falsify_evidence_proof_seal(
    _ingest: &FalsifyEvidenceIngest,
) -> Result<ProofArtifact, EvidenceError> {
    refuse_empirical_proof_seal(ObservationKind::Counterexample)
}

/// Explicit deny path: falsified claims must not become conjecture candidates.
///
/// Callers that hold a [`FalsificationRecord`] must use this gate instead of
/// [`crate::ConjectureCandidate::from_evidence`] on the bridged evidence.
///
/// # Errors
///
/// Always returns [`FalsifyEvidenceError::ConjectureRefused`] when the record
/// is a valid falsification; integrity failure otherwise.
pub fn refuse_conjecture_from_falsification(
    record: &FalsificationRecord,
) -> Result<(), FalsifyEvidenceError> {
    if !record.check_id() {
        return Err(FalsifyEvidenceError::RecordIntegrity);
    }
    if record.status != ClaimStatus::Falsified {
        return Err(FalsifyEvidenceError::NotFalsified);
    }
    Err(FalsifyEvidenceError::ConjectureRefused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClaimBody;
    use crate::falsify::{CheapClaimShape, NatAtom, NatBinOp, NatExpr, try_falsify};

    fn claim(statement: &str) -> Claim {
        Claim::new(ClaimBody {
            statement: statement.into(),
            assumptions: vec![],
            parents: vec![],
        })
    }

    fn falsified_closed() -> (Claim, CounterexampleWitness, FalsificationRecord) {
        let claim = claim("(2 : Nat) + 2 = 5");
        let shape = CheapClaimShape::ClosedEquality {
            left: NatExpr::Bin(NatBinOp::Add, NatAtom::Const(2), NatAtom::Const(2)),
            right: NatExpr::Atom(NatAtom::Const(5)),
        };
        let outcome = try_falsify(claim, shape, None).expect("falsify");
        match outcome {
            crate::FalsificationOutcome::Falsified {
                claim,
                witness,
                record,
            } => (claim, *witness, *record),
            crate::FalsificationOutcome::NotFalsified { .. } => {
                panic!("expected falsified closed equality")
            }
        }
    }

    #[test]
    fn falsification_bridges_to_observed_evidence_without_proved() {
        let (claim, witness, record) = falsified_closed();
        let ingest = evidence_from_falsification(&claim, &witness, &record).expect("bridge");

        assert_eq!(ingest.scientific_status, ClaimStatus::Falsified);
        assert_eq!(record.status, ClaimStatus::Falsified);
        assert_eq!(ingest.evidence.status, ClaimStatus::Observed);
        assert_ne!(ingest.evidence.status, ClaimStatus::Proved);
        assert_ne!(ingest.scientific_status, ClaimStatus::Proved);
        assert_eq!(ingest.observation.kind, ObservationKind::Counterexample);
        assert!(
            ingest
                .observation
                .source_label
                .starts_with("falsify://pl-1.1/")
        );
        assert!(ingest.observation.check_id());
        assert!(ingest.evidence.check_id());
        assert_eq!(ingest.falsification_record_id, record.id);
        assert_eq!(ingest.witness_id, witness.id);
        assert_eq!(ingest.evidence.strength, EvidenceStrength::Strong);
        assert_eq!(ingest.evidence.claim_id, claim.id);
    }

    #[test]
    fn bridge_is_deterministic_for_same_inputs() {
        let (claim, witness, record) = falsified_closed();
        let a = evidence_from_falsification(&claim, &witness, &record).unwrap();
        let b = evidence_from_falsification(&claim, &witness, &record).unwrap();
        assert_eq!(a.observation.id, b.observation.id);
        assert_eq!(a.evidence.id, b.evidence.id);
    }

    #[test]
    fn mismatched_claim_or_witness_fails_closed() {
        let (original, witness, record) = falsified_closed();
        let other = claim("0 = 1");
        assert_eq!(
            evidence_from_falsification(&other, &witness, &record),
            Err(FalsifyEvidenceError::ClaimMismatch)
        );

        let other_shape = CheapClaimShape::ClosedEquality {
            left: NatExpr::Atom(NatAtom::Const(0)),
            right: NatExpr::Atom(NatAtom::Const(1)),
        };
        let other_witness =
            CounterexampleWitness::from_candidate(other_shape, None).expect("witness");
        assert_eq!(
            evidence_from_falsification(&original, &other_witness, &record),
            Err(FalsifyEvidenceError::WitnessMismatch)
        );
    }

    #[test]
    fn tampered_record_status_fails_integrity() {
        let (claim, witness, mut record) = falsified_closed();
        record.status = ClaimStatus::Proved;
        assert_eq!(
            evidence_from_falsification(&claim, &witness, &record),
            Err(FalsifyEvidenceError::RecordIntegrity)
        );
        record.status = ClaimStatus::Observed;
        assert_eq!(
            evidence_from_falsification(&claim, &witness, &record),
            Err(FalsifyEvidenceError::RecordIntegrity)
        );
    }

    #[test]
    fn falsify_bridged_evidence_cannot_seal_proof() {
        let (claim, witness, record) = falsified_closed();
        let ingest = evidence_from_falsification(&claim, &witness, &record).unwrap();
        assert!(refuse_falsify_evidence_proof_seal(&ingest).is_err());
        assert_eq!(
            refuse_empirical_proof_seal(ObservationKind::Counterexample),
            Err(EvidenceError::EmpiricalCannotSealProof("counterexample"))
        );
    }

    #[test]
    fn falsified_record_refuses_conjecture_promotion() {
        let (_claim, _witness, record) = falsified_closed();
        assert_eq!(
            refuse_conjecture_from_falsification(&record),
            Err(FalsifyEvidenceError::ConjectureRefused)
        );
    }

    #[test]
    fn serde_round_trip_preserves_falsified_and_observed() {
        let (claim, witness, record) = falsified_closed();
        let ingest = evidence_from_falsification(&claim, &witness, &record).unwrap();
        let json = serde_json::to_value(&ingest).unwrap();
        let back: FalsifyEvidenceIngest = serde_json::from_value(json).unwrap();
        assert_eq!(back, ingest);
        assert_eq!(back.scientific_status, ClaimStatus::Falsified);
        assert_eq!(back.evidence.status, ClaimStatus::Observed);
        assert!(back.observation.check_id());
        assert!(back.evidence.check_id());

        // Forged upgrade of the ingest wrapper's scientific_status to Proved is
        // visible in JSON; callers must re-validate via the bridge / record rather
        // than trusting deserialized wrapper fields. Evidence remains Observed.
        let mut forged = serde_json::to_value(&ingest).unwrap();
        forged["scientific_status"] = serde_json::json!("Proved");
        let forged_ingest: FalsifyEvidenceIngest = serde_json::from_value(forged).unwrap();
        assert_eq!(forged_ingest.scientific_status, ClaimStatus::Proved);
        assert_ne!(forged_ingest.scientific_status, ClaimStatus::Falsified);
        assert_eq!(forged_ingest.evidence.status, ClaimStatus::Observed);
        assert_ne!(forged_ingest.evidence.status, ClaimStatus::Proved);
        assert!(forged_ingest.evidence.check_id());
    }
}
