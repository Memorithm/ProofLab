//! Cross-oracle finite calibration for explicitly ordered cubic CFI structures.
//!
//! This module executes two already-independent finite oracles on the same
//! caller-supplied ordered CFI inputs: coordinate-wise `k`-WL and the exact
//! bijective/counting-pebble game. Their parameters remain explicit and are not
//! identified with one another. Agreement or disagreement is recorded only as
//! finite computational evidence for the exact inputs and bounds supplied.
//!
//! No outcome from this module establishes an asymptotic lower bound,
//! order-robustness, non-isomorphism, or any separation of complexity classes.
//! `PROVED` remains reserved for `ProofLab`'s configured formal-kernel path.

use core::fmt;

use crate::{
    CfiBaseGraph, CfiBijectivePebbleCalibrationError, CfiTwistAssignment, CfiWlCalibrationError,
    OrderedFiniteStructureId, calibrate_cubic_cfi_bijective_pebble_ordered,
    calibrate_cubic_cfi_wl_ordered,
};

/// Failure while constructing an ordered CFI cross-oracle calibration record.
#[derive(Debug)]
pub enum OrderedCfiCrossCalibrationError {
    /// The ordered `k`-WL calibration failed closed.
    Wl(CfiWlCalibrationError),
    /// The ordered bijective/counting-pebble calibration failed closed.
    Bijective(CfiBijectivePebbleCalibrationError),
    /// Independent adapters produced inconsistent content identities.
    StructureIdentityMismatch,
}

impl fmt::Display for OrderedCfiCrossCalibrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wl(error) => write!(formatter, "ordered CFI WL calibration failed: {error}"),
            Self::Bijective(error) => {
                write!(formatter, "ordered CFI bijective-pebble calibration failed: {error}")
            }
            Self::StructureIdentityMismatch => write!(
                formatter,
                "ordered CFI oracle adapters produced different structure identities"
            ),
        }
    }
}

impl std::error::Error for OrderedCfiCrossCalibrationError {}

impl From<CfiWlCalibrationError> for OrderedCfiCrossCalibrationError {
    fn from(value: CfiWlCalibrationError) -> Self {
        Self::Wl(value)
    }
}

impl From<CfiBijectivePebbleCalibrationError> for OrderedCfiCrossCalibrationError {
    fn from(value: CfiBijectivePebbleCalibrationError) -> Self {
        Self::Bijective(value)
    }
}

/// Finite cross-oracle evidence for one exact pair of ordered CFI structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedCfiCrossCalibration {
    wl_dimension: usize,
    pebble_pairs: usize,
    rounds: u32,
    left_structure: OrderedFiniteStructureId,
    right_structure: OrderedFiniteStructureId,
    wl_distinguished: bool,
    bijective_duplicator_wins: bool,
    outcomes_agree: bool,
    wl_refinement_applications: usize,
    wl_stable_color_classes: usize,
    pebble_states_explored: u64,
    pebble_bijections_considered: u64,
}

impl OrderedCfiCrossCalibration {
    /// Dimension supplied to coordinate-wise WL.
    #[must_use]
    pub const fn wl_dimension(&self) -> usize {
        self.wl_dimension
    }

    /// Number of reusable pebble pairs supplied to the bijective game.
    #[must_use]
    pub const fn pebble_pairs(&self) -> usize {
        self.pebble_pairs
    }

    /// Round bound supplied to the bijective game.
    #[must_use]
    pub const fn rounds(&self) -> u32 {
        self.rounds
    }

    /// Content identity of the exact left ordered structure.
    #[must_use]
    pub const fn left_structure(&self) -> OrderedFiniteStructureId {
        self.left_structure
    }

    /// Content identity of the exact right ordered structure.
    #[must_use]
    pub const fn right_structure(&self) -> OrderedFiniteStructureId {
        self.right_structure
    }

    /// Whether the WL oracle distinguished the exact ordered pair.
    #[must_use]
    pub const fn wl_distinguished(&self) -> bool {
        self.wl_distinguished
    }

    /// Whether Duplicator won the bounded exact bijective game.
    #[must_use]
    pub const fn bijective_duplicator_wins(&self) -> bool {
        self.bijective_duplicator_wins
    }

    /// Whether both finite oracles returned the same distinguishability polarity.
    ///
    /// This is an observed equality only. No correspondence between the chosen
    /// WL dimension and pebble/round parameters is inferred from it.
    #[must_use]
    pub const fn outcomes_agree(&self) -> bool {
        self.outcomes_agree
    }

    /// Number of WL refinement applications, including the stable pass.
    #[must_use]
    pub const fn wl_refinement_applications(&self) -> usize {
        self.wl_refinement_applications
    }

    /// Stable WL color classes for the exact joint ordered comparison.
    #[must_use]
    pub const fn wl_stable_color_classes(&self) -> usize {
        self.wl_stable_color_classes
    }

    /// Previously unseen states explored by the exact bijective game solver.
    #[must_use]
    pub const fn pebble_states_explored(&self) -> u64 {
        self.pebble_states_explored
    }

    /// Candidate bijections considered by the exact bijective game solver.
    #[must_use]
    pub const fn pebble_bijections_considered(&self) -> u64 {
        self.pebble_bijections_considered
    }
}

/// Execute ordered WL and ordered bijective/counting-pebble calibration on the
/// same exact cubic CFI input pair.
///
/// `wl_dimension`, `pebble_pairs`, and `rounds` are intentionally independent
/// experimental parameters. Callers must not infer a theoretical equivalence
/// from equal or unequal outcomes without a separate formal argument.
///
/// # Errors
///
/// Fails closed if either underlying oracle rejects the CFI/order inputs, or if
/// their independently constructed ordered-structure content identities differ.
pub fn calibrate_cubic_cfi_ordered_cross_oracle(
    base: &CfiBaseGraph,
    left_twists: &CfiTwistAssignment,
    right_twists: &CfiTwistAssignment,
    left_order: Vec<u64>,
    right_order: Vec<u64>,
    wl_dimension: usize,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<OrderedCfiCrossCalibration, OrderedCfiCrossCalibrationError> {
    let wl = calibrate_cubic_cfi_wl_ordered(
        base,
        left_twists,
        right_twists,
        left_order.clone(),
        right_order.clone(),
        wl_dimension,
    )?;
    let pebble = calibrate_cubic_cfi_bijective_pebble_ordered(
        base,
        left_twists,
        right_twists,
        left_order,
        right_order,
        pebble_pairs,
        rounds,
    )?;

    if wl.left_structure() != pebble.left_structure()
        || wl.right_structure() != pebble.right_structure()
    {
        return Err(OrderedCfiCrossCalibrationError::StructureIdentityMismatch);
    }

    let wl_indistinguished = !wl.distinguished();
    let outcomes_agree = wl_indistinguished == pebble.duplicator_wins();

    Ok(OrderedCfiCrossCalibration {
        wl_dimension,
        pebble_pairs,
        rounds,
        left_structure: wl.left_structure(),
        right_structure: wl.right_structure(),
        wl_distinguished: wl.distinguished(),
        bijective_duplicator_wins: pebble.duplicator_wins(),
        outcomes_agree,
        wl_refinement_applications: wl.refinement_applications(),
        wl_stable_color_classes: wl.stable_color_classes(),
        pebble_states_explored: pebble.states_explored(),
        pebble_bijections_considered: pebble.bijections_considered(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k4() -> CfiBaseGraph {
        CfiBaseGraph::new(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]).unwrap()
    }

    fn natural_order(size: u64) -> Vec<u64> {
        (0..size).collect()
    }

    #[test]
    fn identical_ordered_input_is_a_cross_oracle_control() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let order = natural_order(40);
        let record = calibrate_cubic_cfi_ordered_cross_oracle(
            &base,
            &twists,
            &twists,
            order.clone(),
            order,
            2,
            1,
            1,
        )
        .unwrap();

        assert!(!record.wl_distinguished());
        assert!(record.bijective_duplicator_wins());
        assert!(record.outcomes_agree());
        assert_eq!(record.left_structure(), record.right_structure());
    }

    #[test]
    fn records_parameters_without_identifying_them() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let order = natural_order(40);
        let record = calibrate_cubic_cfi_ordered_cross_oracle(
            &base,
            &twists,
            &twists,
            order.clone(),
            order,
            3,
            1,
            0,
        )
        .unwrap();

        assert_eq!(record.wl_dimension(), 3);
        assert_eq!(record.pebble_pairs(), 1);
        assert_eq!(record.rounds(), 0);
    }
}
