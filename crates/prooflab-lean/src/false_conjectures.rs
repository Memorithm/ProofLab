//! PL-1.1 controlled false conjectures.
//!
//! Intentionally false Nat statements ship with known finite counterexamples.
//! Cheap falsification must reject them and refuse proof-search promotion before
//! any Lean kernel invocation. Lean remains the only authority that may seal
//! [`prooflab_core::ClaimStatus::Proved`]; a falsification record never does.

use std::fmt;

use prooflab_core::{
    CheapClaimShape, Claim, ClaimBody, ClaimStatus, FalsificationOutcome, FalsifyError, NatAtom,
    NatBinOp, NatExpr, refuse_proof_search, try_falsify,
};

/// One intentionally false conjecture with a known cheap counterexample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FalseConjectureEntry {
    pub id: &'static str,
    pub statement: &'static str,
    pub assumptions: &'static [&'static str],
    pub shape: CheapClaimShape,
    /// Instantiation of `n` for universal shapes; `None` for closed equations.
    pub witness_n: Option<u64>,
}

/// Measured cheap-falsification result for one controlled false conjecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FalseConjectureReport {
    pub entry_id: &'static str,
    pub falsified: bool,
    pub proof_search_blocked: bool,
    pub status: Option<ClaimStatus>,
    pub witness_n: Option<u64>,
    pub left_value: Option<u64>,
    pub right_value: Option<u64>,
}

/// Fail-closed orchestration error for the PL-1.1 battery.
#[derive(Debug)]
pub enum FalseConjectureError {
    Falsify(FalsifyError),
    ExpectedFalsification(&'static str),
    ProofSearchNotBlocked(&'static str),
}

impl fmt::Display for FalseConjectureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Falsify(error) => write!(formatter, "{error}"),
            Self::ExpectedFalsification(id) => {
                write!(
                    formatter,
                    "controlled false conjecture {id} was not falsified by its known witness"
                )
            }
            Self::ProofSearchNotBlocked(id) => {
                write!(
                    formatter,
                    "controlled false conjecture {id} did not block proof-search promotion"
                )
            }
        }
    }
}

impl std::error::Error for FalseConjectureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Falsify(error) => Some(error),
            Self::ExpectedFalsification(_) | Self::ProofSearchNotBlocked(_) => None,
        }
    }
}

impl From<FalsifyError> for FalseConjectureError {
    fn from(error: FalsifyError) -> Self {
        Self::Falsify(error)
    }
}

/// Controlled false conjectures used to measure cheap falsification routing.
///
/// These statements are intentionally false. Matching the expected falsification
/// outcome measures orchestration only and does not claim mathematical novelty.
pub const CONTROLLED_FALSE_CONJECTURES: &[FalseConjectureEntry] = &[
    FalseConjectureEntry {
        id: "closed-two-plus-two-eq-five",
        statement: "(2 : Nat) + 2 = 5",
        assumptions: &[],
        shape: CheapClaimShape::ClosedEquality {
            left: NatExpr::Bin(NatBinOp::Add, NatAtom::Const(2), NatAtom::Const(2)),
            right: NatExpr::Atom(NatAtom::Const(5)),
        },
        witness_n: None,
    },
    FalseConjectureEntry {
        id: "universal-n-plus-one-eq-n",
        statement: "forall n : Nat, n + 1 = n",
        assumptions: &["n : Nat"],
        shape: CheapClaimShape::UniversalEquality {
            left: NatExpr::Bin(NatBinOp::Add, NatAtom::Var, NatAtom::Const(1)),
            right: NatExpr::Atom(NatAtom::Var),
        },
        witness_n: Some(0),
    },
    FalseConjectureEntry {
        id: "universal-n-eq-zero",
        statement: "forall n : Nat, n = 0",
        assumptions: &["n : Nat"],
        shape: CheapClaimShape::UniversalEquality {
            left: NatExpr::Atom(NatAtom::Var),
            right: NatExpr::Atom(NatAtom::Const(0)),
        },
        witness_n: Some(1),
    },
    FalseConjectureEntry {
        id: "universal-n-mul-zero-eq-n",
        statement: "forall n : Nat, n * 0 = n",
        assumptions: &["n : Nat"],
        shape: CheapClaimShape::UniversalEquality {
            left: NatExpr::Bin(NatBinOp::Mul, NatAtom::Var, NatAtom::Const(0)),
            right: NatExpr::Atom(NatAtom::Var),
        },
        witness_n: Some(1),
    },
    FalseConjectureEntry {
        id: "universal-succ-n-eq-n",
        statement: "forall n : Nat, Nat.succ n = n",
        assumptions: &["n : Nat"],
        shape: CheapClaimShape::UniversalEquality {
            left: NatExpr::Succ(NatAtom::Var),
            right: NatExpr::Atom(NatAtom::Var),
        },
        witness_n: Some(0),
    },
];

impl FalseConjectureEntry {
    /// Build the immutable claim for this false conjecture.
    #[must_use]
    pub fn claim(&self) -> Claim {
        Claim::new(ClaimBody {
            statement: self.statement.into(),
            assumptions: self
                .assumptions
                .iter()
                .map(|assumption| (*assumption).to_owned())
                .collect(),
            parents: vec![],
        })
    }

    /// Run the cheap falsification route for this entry.
    ///
    /// # Errors
    ///
    /// Propagates evaluation failures from the core falsifier.
    pub fn falsify(&self) -> Result<FalsificationOutcome, FalsifyError> {
        try_falsify(self.claim(), self.shape, self.witness_n)
    }

    /// Gate proof-search promotion: falsified entries must be refused.
    ///
    /// On success, returns the falsification outcome and guarantees
    /// [`FalsificationOutcome::blocks_proof_search`] is true. Callers must not
    /// invoke [`crate::LeanKernel`] for proof search after this gate.
    ///
    /// # Errors
    ///
    /// Fails when the known witness does not falsify the entry or when the
    /// proof-search refusal check fails.
    pub fn refuse_proof_search_promotion(
        &self,
    ) -> Result<FalsificationOutcome, FalseConjectureError> {
        let outcome = self.falsify()?;
        if !outcome.blocks_proof_search() {
            return Err(FalseConjectureError::ExpectedFalsification(self.id));
        }
        // The refusal helper returns Err precisely when promotion is blocked.
        match refuse_proof_search(&outcome) {
            Err(_) => Ok(outcome),
            Ok(()) => Err(FalseConjectureError::ProofSearchNotBlocked(self.id)),
        }
    }
}

/// Run the full PL-1.1 controlled false-conjecture battery.
///
/// Every entry must be falsified by its known witness and must block
/// proof-search promotion. Lean is not invoked.
///
/// # Errors
///
/// Fails closed on the first entry that is not falsified or fails to block
/// proof search.
pub fn run_controlled_false_conjecture_battery()
-> Result<Vec<FalseConjectureReport>, FalseConjectureError> {
    CONTROLLED_FALSE_CONJECTURES
        .iter()
        .map(|entry| {
            let outcome = entry.refuse_proof_search_promotion()?;
            let report = match &outcome {
                FalsificationOutcome::Falsified {
                    witness, record, ..
                } => FalseConjectureReport {
                    entry_id: entry.id,
                    falsified: true,
                    proof_search_blocked: true,
                    status: Some(record.status),
                    witness_n: witness.n,
                    left_value: Some(witness.left_value),
                    right_value: Some(witness.right_value),
                },
                FalsificationOutcome::NotFalsified { .. } => {
                    return Err(FalseConjectureError::ExpectedFalsification(entry.id));
                }
            };
            Ok(report)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_falsifies_all_entries_without_lean() {
        let reports = run_controlled_false_conjecture_battery().expect("battery");
        assert!(
            reports.len() >= 4,
            "PL-1.1 needs several controlled false conjectures"
        );
        for report in &reports {
            assert!(report.falsified, "{} not falsified", report.entry_id);
            assert!(
                report.proof_search_blocked,
                "{} did not block proof search",
                report.entry_id
            );
            assert_eq!(report.status, Some(ClaimStatus::Falsified));
            assert_ne!(report.left_value, report.right_value);
        }
    }

    #[test]
    fn closed_and_universal_shapes_are_both_covered() {
        let closed = CONTROLLED_FALSE_CONJECTURES
            .iter()
            .filter(|entry| matches!(entry.shape, CheapClaimShape::ClosedEquality { .. }))
            .count();
        let universal = CONTROLLED_FALSE_CONJECTURES
            .iter()
            .filter(|entry| matches!(entry.shape, CheapClaimShape::UniversalEquality { .. }))
            .count();
        assert!(closed >= 1);
        assert!(universal >= 3);
    }

    #[test]
    fn gate_never_authorizes_proved_status() {
        for entry in CONTROLLED_FALSE_CONJECTURES {
            let outcome = entry.refuse_proof_search_promotion().expect("gate");
            assert_eq!(outcome.claim_status(), Some(ClaimStatus::Falsified));
            assert_ne!(outcome.claim_status(), Some(ClaimStatus::Proved));
        }
    }

    #[test]
    fn claim_identity_stable_per_entry() {
        let first = CONTROLLED_FALSE_CONJECTURES[0].claim();
        let second = CONTROLLED_FALSE_CONJECTURES[0].claim();
        assert_eq!(first.id, second.id);
        assert_ne!(first.id, CONTROLLED_FALSE_CONJECTURES[1].claim().id);
    }
}
