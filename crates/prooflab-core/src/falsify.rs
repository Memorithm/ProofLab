//! Cheap, deterministic falsification for controlled false conjectures (PL-1.1).
//!
//! A verified counterexample may justify [`ClaimStatus::Falsified`]. It never
//! authorizes [`ClaimStatus::Proved`]. Absence of a cheap counterexample likewise
//! never authorizes proof status: only a trusted-kernel
//! [`crate::ProofArtifact`] may seal proof.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::canonical::{Canonical, CanonicalEncoder, sha256_canonical};
use crate::{Claim, ClaimId, ClaimStatus};

const COUNTEREXAMPLE_WITNESS_DOMAIN: &[u8] = b"prooflab-counterexample-witness:v1\0";
const FALSIFICATION_RECORD_DOMAIN: &[u8] = b"prooflab-falsification-record:v1\0";

/// Fail-closed falsification error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalsifyError {
    UnboundVariable,
    Overflow,
    ShapeMismatch,
    WitnessDoesNotFalsify,
    ClaimIdentityMismatch,
    /// Cheap falsification already rejected the claim; proof search must not proceed.
    ProofSearchBlocked,
}

impl fmt::Display for FalsifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundVariable => {
                write!(
                    formatter,
                    "Nat expression references n without a witness value"
                )
            }
            Self::Overflow => write!(formatter, "Nat evaluation overflowed"),
            Self::ShapeMismatch => {
                write!(
                    formatter,
                    "counterexample witness shape does not match the claim shape"
                )
            }
            Self::WitnessDoesNotFalsify => {
                write!(formatter, "candidate witness does not falsify the claim")
            }
            Self::ClaimIdentityMismatch => {
                write!(
                    formatter,
                    "falsification record claim id does not match claim"
                )
            }
            Self::ProofSearchBlocked => write!(
                formatter,
                "proof-search promotion refused: claim was falsified by a cheap counterexample"
            ),
        }
    }
}

impl std::error::Error for FalsifyError {}

/// Atomic Nat value in the restricted PL-1.1 expression language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NatAtom {
    Const(u64),
    /// The single free variable `n` in a universal claim.
    Var,
}

/// Binary Nat operators admitted by the cheap falsifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NatBinOp {
    Add,
    Mul,
}

/// Restricted Nat expressions (atoms, binary ops of atoms, or successor of an atom).
///
/// Richer syntax belongs in a later counterexample crate. This subset is enough
/// for intentionally false Nat equalities with known finite witnesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NatExpr {
    Atom(NatAtom),
    Bin(NatBinOp, NatAtom, NatAtom),
    Succ(NatAtom),
}

impl NatAtom {
    fn eval(self, n: Option<u64>) -> Result<u64, FalsifyError> {
        match self {
            Self::Const(value) => Ok(value),
            Self::Var => n.ok_or(FalsifyError::UnboundVariable),
        }
    }
}

impl NatExpr {
    /// Evaluate under an optional instantiation of `n`.
    ///
    /// # Errors
    ///
    /// Returns [`FalsifyError::UnboundVariable`] when `Var` appears without `n`,
    /// or [`FalsifyError::Overflow`] on wrapping arithmetic.
    pub fn eval(self, n: Option<u64>) -> Result<u64, FalsifyError> {
        match self {
            Self::Atom(atom) => atom.eval(n),
            Self::Bin(op, left, right) => {
                let left = left.eval(n)?;
                let right = right.eval(n)?;
                match op {
                    NatBinOp::Add => left.checked_add(right).ok_or(FalsifyError::Overflow),
                    NatBinOp::Mul => left.checked_mul(right).ok_or(FalsifyError::Overflow),
                }
            }
            Self::Succ(atom) => atom.eval(n)?.checked_add(1).ok_or(FalsifyError::Overflow),
        }
    }
}

/// Claim shapes that admit a sound, cheap finite counterexample check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CheapClaimShape {
    /// Closed equation `left = right` with no free variables.
    ClosedEquality { left: NatExpr, right: NatExpr },
    /// Universal equation `∀ n, left(n) = right(n)`.
    UniversalEquality { left: NatExpr, right: NatExpr },
}

impl CheapClaimShape {
    /// Evaluate both sides under `n` (ignored for closed equations).
    ///
    /// # Errors
    ///
    /// Propagates evaluation failures from [`NatExpr::eval`].
    pub fn evaluate_sides(self, n: Option<u64>) -> Result<(u64, u64), FalsifyError> {
        match self {
            Self::ClosedEquality { left, right } => Ok((left.eval(None)?, right.eval(None)?)),
            Self::UniversalEquality { left, right } => Ok((left.eval(n)?, right.eval(n)?)),
        }
    }

    /// Return whether the sides differ under `n`.
    ///
    /// # Errors
    ///
    /// Propagates evaluation failures. For universal shapes, `n` must be `Some`.
    pub fn is_falsified_under(self, n: Option<u64>) -> Result<bool, FalsifyError> {
        match self {
            Self::ClosedEquality { .. } => {
                let (left, right) = self.evaluate_sides(None)?;
                Ok(left != right)
            }
            Self::UniversalEquality { .. } => {
                if n.is_none() {
                    return Err(FalsifyError::UnboundVariable);
                }
                let (left, right) = self.evaluate_sides(n)?;
                Ok(left != right)
            }
        }
    }
}

/// Stable identity of an immutable counterexample witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CounterexampleWitnessId(pub [u8; 32]);

/// Content-addressed finite witness that refutes a cheap claim shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterexampleWitness {
    pub id: CounterexampleWitnessId,
    pub shape: CheapClaimShape,
    /// Instantiation of `n` for universal shapes; `None` for closed equations.
    pub n: Option<u64>,
    pub left_value: u64,
    pub right_value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CounterexampleWitnessBody {
    shape: CheapClaimShape,
    n: Option<u64>,
    left_value: u64,
    right_value: u64,
}

impl Canonical for CounterexampleWitnessBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str(match self.shape {
            CheapClaimShape::ClosedEquality { .. } => "closed-eq",
            CheapClaimShape::UniversalEquality { .. } => "universal-eq",
        });
        encode_shape(encoder, self.shape);
        match self.n {
            Some(value) => {
                encoder.bool(true);
                encoder.u64(value);
            }
            None => encoder.bool(false),
        }
        encoder.u64(self.left_value);
        encoder.u64(self.right_value);
    }
}

impl Canonical for NatAtom {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        match self {
            Self::Const(value) => {
                encoder.str("const");
                encoder.u64(*value);
            }
            Self::Var => encoder.str("var"),
        }
    }
}

impl Canonical for NatExpr {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        match self {
            Self::Atom(atom) => {
                encoder.str("atom");
                encoder.value(atom);
            }
            Self::Bin(op, left, right) => {
                encoder.str("bin");
                encoder.str(match op {
                    NatBinOp::Add => "add",
                    NatBinOp::Mul => "mul",
                });
                encoder.value(left);
                encoder.value(right);
            }
            Self::Succ(atom) => {
                encoder.str("succ");
                encoder.value(atom);
            }
        }
    }
}

fn encode_shape(encoder: &mut CanonicalEncoder, shape: CheapClaimShape) {
    match shape {
        CheapClaimShape::ClosedEquality { left, right }
        | CheapClaimShape::UniversalEquality { left, right } => {
            encoder.value(&left);
            encoder.value(&right);
        }
    }
}

impl CounterexampleWitness {
    /// Build a witness by evaluating `shape` under `n`.
    ///
    /// # Errors
    ///
    /// Fails closed when evaluation fails or the sides are equal (not a counterexample).
    pub fn from_candidate(shape: CheapClaimShape, n: Option<u64>) -> Result<Self, FalsifyError> {
        if !shape.is_falsified_under(n)? {
            return Err(FalsifyError::WitnessDoesNotFalsify);
        }
        let (left_value, right_value) = shape.evaluate_sides(match shape {
            CheapClaimShape::ClosedEquality { .. } => None,
            CheapClaimShape::UniversalEquality { .. } => n,
        })?;
        let body = CounterexampleWitnessBody {
            shape,
            n: match shape {
                CheapClaimShape::ClosedEquality { .. } => None,
                CheapClaimShape::UniversalEquality { .. } => n,
            },
            left_value,
            right_value,
        };
        Ok(Self {
            id: CounterexampleWitnessId(sha256_canonical(COUNTEREXAMPLE_WITNESS_DOMAIN, &body)),
            shape: body.shape,
            n: body.n,
            left_value: body.left_value,
            right_value: body.right_value,
        })
    }

    /// Return whether the stored content address still matches the fields.
    #[must_use]
    pub fn check_id(&self) -> bool {
        let body = CounterexampleWitnessBody {
            shape: self.shape,
            n: self.n,
            left_value: self.left_value,
            right_value: self.right_value,
        };
        self.id == CounterexampleWitnessId(sha256_canonical(COUNTEREXAMPLE_WITNESS_DOMAIN, &body))
    }

    /// Re-check that this witness still falsifies its shape.
    ///
    /// # Errors
    ///
    /// Returns [`FalsifyError::WitnessDoesNotFalsify`] when the recorded values
    /// no longer disagree, or evaluation errors.
    pub fn validate(&self) -> Result<(), FalsifyError> {
        if !self.check_id() {
            return Err(FalsifyError::WitnessDoesNotFalsify);
        }
        if !self.shape.is_falsified_under(self.n)? {
            return Err(FalsifyError::WitnessDoesNotFalsify);
        }
        let (left, right) = self.shape.evaluate_sides(self.n)?;
        if left != self.left_value || right != self.right_value || left == right {
            return Err(FalsifyError::WitnessDoesNotFalsify);
        }
        Ok(())
    }
}

/// Stable identity of an immutable falsification record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FalsificationRecordId(pub [u8; 32]);

/// Content-addressed record that a claim was rejected by a cheap counterexample.
///
/// Binding a record never seals [`ClaimStatus::Proved`]. Scientific status for
/// the associated claim becomes [`ClaimStatus::Falsified`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FalsificationRecord {
    pub id: FalsificationRecordId,
    pub claim_id: ClaimId,
    pub witness_id: CounterexampleWitnessId,
    pub status: ClaimStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FalsificationRecordBody {
    claim_id: ClaimId,
    witness_id: CounterexampleWitnessId,
    status: ClaimStatus,
}

impl Canonical for CounterexampleWitnessId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for FalsificationRecordBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.claim_id);
        encoder.value(&self.witness_id);
        encoder.str(match self.status {
            ClaimStatus::Falsified => "falsified",
            ClaimStatus::Observed => "observed",
            ClaimStatus::Conjectured => "conjectured",
            ClaimStatus::Formalized => "formalized",
            ClaimStatus::Proved => "proved",
            ClaimStatus::Generalized => "generalized",
        });
    }
}

impl FalsificationRecord {
    /// Seal a falsification after validating the witness against `claim`.
    ///
    /// # Errors
    ///
    /// Fails closed when the witness is invalid. Always records
    /// [`ClaimStatus::Falsified`]; never [`ClaimStatus::Proved`].
    pub fn new(claim: &Claim, witness: &CounterexampleWitness) -> Result<Self, FalsifyError> {
        witness.validate()?;
        let body = FalsificationRecordBody {
            claim_id: claim.id,
            witness_id: witness.id,
            status: ClaimStatus::Falsified,
        };
        Ok(Self {
            id: FalsificationRecordId(sha256_canonical(FALSIFICATION_RECORD_DOMAIN, &body)),
            claim_id: body.claim_id,
            witness_id: body.witness_id,
            status: ClaimStatus::Falsified,
        })
    }

    /// Return whether the stored content address still matches the fields.
    #[must_use]
    pub fn check_id(&self) -> bool {
        if self.status != ClaimStatus::Falsified {
            return false;
        }
        let body = FalsificationRecordBody {
            claim_id: self.claim_id,
            witness_id: self.witness_id,
            status: self.status,
        };
        self.id == FalsificationRecordId(sha256_canonical(FALSIFICATION_RECORD_DOMAIN, &body))
    }
}

/// Outcome of attempting cheap falsification before proof search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalsificationOutcome {
    /// Claim is rejected; proof search must not be promoted.
    Falsified {
        claim: Claim,
        witness: Box<CounterexampleWitness>,
        record: Box<FalsificationRecord>,
    },
    /// No cheap counterexample under the supplied candidate.
    ///
    /// This does **not** authorize [`ClaimStatus::Proved`].
    NotFalsified { claim: Claim },
}

impl FalsificationOutcome {
    /// Return whether proof-search promotion is blocked.
    #[must_use]
    pub const fn blocks_proof_search(&self) -> bool {
        matches!(self, Self::Falsified { .. })
    }

    /// Scientific status implied by this outcome, when falsified.
    #[must_use]
    pub const fn claim_status(&self) -> Option<ClaimStatus> {
        match self {
            Self::Falsified { .. } => Some(ClaimStatus::Falsified),
            Self::NotFalsified { .. } => None,
        }
    }
}

/// Try to falsify `claim` under `shape` with a candidate instantiation.
///
/// # Errors
///
/// Propagates evaluation failures. A non-falsifying candidate yields
/// [`FalsificationOutcome::NotFalsified`], not an error.
pub fn try_falsify(
    claim: Claim,
    shape: CheapClaimShape,
    n: Option<u64>,
) -> Result<FalsificationOutcome, FalsifyError> {
    match CounterexampleWitness::from_candidate(shape, n) {
        Ok(witness) => {
            let record = FalsificationRecord::new(&claim, &witness)?;
            Ok(FalsificationOutcome::Falsified {
                claim,
                witness: Box::new(witness),
                record: Box::new(record),
            })
        }
        Err(FalsifyError::WitnessDoesNotFalsify) => {
            Ok(FalsificationOutcome::NotFalsified { claim })
        }
        Err(error) => Err(error),
    }
}

/// Refuse proof-search promotion when a falsification outcome blocked it.
///
/// # Errors
///
/// Returns [`FalsifyError::WitnessDoesNotFalsify`] when called on a falsified
/// outcome (the caller must not proceed). Prefer checking
/// [`FalsificationOutcome::blocks_proof_search`] first; this helper is the
/// fail-closed gate used by orchestration tests.
pub fn refuse_proof_search(outcome: &FalsificationOutcome) -> Result<(), FalsifyError> {
    if outcome.blocks_proof_search() {
        Err(FalsifyError::ProofSearchBlocked)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClaimBody;

    fn claim(statement: &str) -> Claim {
        Claim::new(ClaimBody {
            statement: statement.into(),
            assumptions: vec![],
            parents: vec![],
        })
    }

    #[test]
    fn closed_false_equation_is_falsified_without_n() {
        let shape = CheapClaimShape::ClosedEquality {
            left: NatExpr::Bin(NatBinOp::Add, NatAtom::Const(2), NatAtom::Const(2)),
            right: NatExpr::Atom(NatAtom::Const(5)),
        };
        let outcome = try_falsify(claim("(2 : Nat) + 2 = 5"), shape, None).unwrap();
        assert!(outcome.blocks_proof_search());
        assert_eq!(outcome.claim_status(), Some(ClaimStatus::Falsified));
        let FalsificationOutcome::Falsified {
            witness, record, ..
        } = outcome
        else {
            panic!("expected falsified");
        };
        assert!(witness.check_id());
        assert!(record.check_id());
        assert_eq!(record.status, ClaimStatus::Falsified);
        assert_eq!(witness.left_value, 4);
        assert_eq!(witness.right_value, 5);
    }

    #[test]
    fn universal_false_equation_uses_known_witness() {
        let shape = CheapClaimShape::UniversalEquality {
            left: NatExpr::Bin(NatBinOp::Add, NatAtom::Var, NatAtom::Const(1)),
            right: NatExpr::Atom(NatAtom::Var),
        };
        let outcome = try_falsify(claim("forall n : Nat, n + 1 = n"), shape, Some(0)).unwrap();
        assert!(outcome.blocks_proof_search());
        refuse_proof_search(&outcome).expect_err("must block proof search");
    }

    #[test]
    fn true_universal_equation_is_not_falsified_by_any_small_n() {
        let shape = CheapClaimShape::UniversalEquality {
            left: NatExpr::Atom(NatAtom::Var),
            right: NatExpr::Atom(NatAtom::Var),
        };
        for n in 0..8 {
            let outcome = try_falsify(claim("forall n : Nat, n = n"), shape, Some(n)).unwrap();
            assert!(!outcome.blocks_proof_search());
            refuse_proof_search(&outcome).unwrap();
        }
    }

    #[test]
    fn wrong_witness_does_not_falsify_vacuously_true_instance() {
        // ∀ n, n * 0 = 0 is true; n=3 still holds, so not a counterexample.
        let shape = CheapClaimShape::UniversalEquality {
            left: NatExpr::Bin(NatBinOp::Mul, NatAtom::Var, NatAtom::Const(0)),
            right: NatExpr::Atom(NatAtom::Const(0)),
        };
        let outcome = try_falsify(claim("forall n : Nat, n * 0 = 0"), shape, Some(3)).unwrap();
        assert!(!outcome.blocks_proof_search());
    }

    #[test]
    fn falsification_never_records_proved_status() {
        let shape = CheapClaimShape::ClosedEquality {
            left: NatExpr::Atom(NatAtom::Const(0)),
            right: NatExpr::Atom(NatAtom::Const(1)),
        };
        let outcome = try_falsify(claim("0 = 1"), shape, None).unwrap();
        let FalsificationOutcome::Falsified { record, .. } = outcome else {
            panic!("expected falsified");
        };
        assert_ne!(record.status, ClaimStatus::Proved);
        assert_eq!(record.status, ClaimStatus::Falsified);
    }
}
