//! Explicit-order CFI-to-bijective-pebble calibration records for PL-DC.
//!
//! Results from this module are finite computational evidence for exact
//! caller-supplied ordered expansions only. They do not establish an
//! order-robust lower bound, non-isomorphism, or any separation of complexity
//! classes, and they are never promoted to `PROVED` without ProofLab's formal
//! kernel path.

use crate::{
    CfiBaseGraph, CfiBijectivePebbleCalibrationError, CfiTwistAssignment,
    OrderedFiniteStructure, OrderedFiniteStructureId, cubic_cfi_as_relational,
    solve_bijective_pebble_ordered,
};

/// Deterministic finite calibration record for one exact pair of ordered CFI
/// structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedCfiBijectivePebbleCalibration {
    pebble_pairs: usize,
    rounds: u32,
    left_structure: OrderedFiniteStructureId,
    right_structure: OrderedFiniteStructureId,
    left_twist_count: usize,
    right_twist_count: usize,
    left_odd_parity: bool,
    right_odd_parity: bool,
    duplicator_wins: bool,
    states_explored: u64,
    bijections_considered: u64,
}

impl OrderedCfiBijectivePebbleCalibration {
    /// Number of reusable pebble pairs in the exact finite game.
    #[must_use]
    pub const fn pebble_pairs(&self) -> usize {
        self.pebble_pairs
    }

    /// Requested number of game rounds.
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

    /// Number of twisted base edges on the left.
    #[must_use]
    pub const fn left_twist_count(&self) -> usize {
        self.left_twist_count
    }

    /// Number of twisted base edges on the right.
    #[must_use]
    pub const fn right_twist_count(&self) -> usize {
        self.right_twist_count
    }

    /// Whether the left twist assignment has odd parity.
    #[must_use]
    pub const fn left_odd_parity(&self) -> bool {
        self.left_odd_parity
    }

    /// Whether the right twist assignment has odd parity.
    #[must_use]
    pub const fn right_odd_parity(&self) -> bool {
        self.right_odd_parity
    }

    /// Observed outcome for these exact ordered expansions only.
    #[must_use]
    pub const fn duplicator_wins(&self) -> bool {
        self.duplicator_wins
    }

    /// Number of previously unseen game states explored by the exact solver.
    #[must_use]
    pub const fn states_explored(&self) -> u64 {
        self.states_explored
    }

    /// Number of candidate bijections considered by the exact solver.
    #[must_use]
    pub const fn bijections_considered(&self) -> u64 {
        self.bijections_considered
    }
}

/// Compare two cubic CFI twist assignments after attaching exact
/// caller-supplied total orders to their expanded carriers.
///
/// The returned outcome is finite calibration evidence for the supplied base,
/// twists, orders, pebble count, and round bound only. In particular, one
/// chosen order pair cannot establish robustness to arbitrary ordered
/// expansions.
///
/// # Errors
///
/// Fails closed on invalid CFI construction, relational translation, malformed
/// order permutations, or an exact bijective-pebble solver error.
pub fn calibrate_cubic_cfi_bijective_pebble_ordered(
    base: &CfiBaseGraph,
    left_twists: &CfiTwistAssignment,
    right_twists: &CfiTwistAssignment,
    left_order: Vec<u64>,
    right_order: Vec<u64>,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<OrderedCfiBijectivePebbleCalibration, CfiBijectivePebbleCalibrationError> {
    let left_graph = crate::CubicCfiGraph::new(base, left_twists)?;
    let right_graph = crate::CubicCfiGraph::new(base, right_twists)?;
    let left = cubic_cfi_as_relational(&left_graph)?;
    let right = cubic_cfi_as_relational(&right_graph)?;
    let left = OrderedFiniteStructure::new(left, left_order)
        .map_err(CfiBijectivePebbleCalibrationError::Descriptive)?;
    let right = OrderedFiniteStructure::new(right, right_order)
        .map_err(CfiBijectivePebbleCalibrationError::Descriptive)?;
    let result = solve_bijective_pebble_ordered(&left, &right, pebble_pairs, rounds)?;

    Ok(OrderedCfiBijectivePebbleCalibration {
        pebble_pairs,
        rounds,
        left_structure: left.id(),
        right_structure: right.id(),
        left_twist_count: left_twists.twist_count(),
        right_twist_count: right_twists.twist_count(),
        left_odd_parity: left_twists.odd_parity(),
        right_odd_parity: right_twists.odd_parity(),
        duplicator_wins: result.duplicator_wins(),
        states_explored: result.states_explored(),
        bijections_considered: result.bijections_considered(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DescriptiveError;

    fn k4() -> CfiBaseGraph {
        CfiBaseGraph::new(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]).unwrap()
    }

    fn natural_order(size: u64) -> Vec<u64> {
        (0..size).collect()
    }

    #[test]
    fn identical_ordered_structure_is_a_deterministic_control() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let order = natural_order(40);
        let first = calibrate_cubic_cfi_bijective_pebble_ordered(
            &base,
            &twists,
            &twists,
            order.clone(),
            order.clone(),
            1,
            1,
        )
        .unwrap();
        let second = calibrate_cubic_cfi_bijective_pebble_ordered(
            &base, &twists, &twists, order.clone(), order, 1, 1,
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(first.duplicator_wins());
        assert_eq!(first.left_structure(), first.right_structure());
    }

    #[test]
    fn distinguished_order_is_part_of_the_content_identity() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let left_order = natural_order(40);
        let mut right_order = left_order.clone();
        right_order.swap(0, 1);
        let record = calibrate_cubic_cfi_bijective_pebble_ordered(
            &base,
            &twists,
            &twists,
            left_order,
            right_order,
            1,
            0,
        )
        .unwrap();
        assert_ne!(record.left_structure(), record.right_structure());
    }

    #[test]
    fn malformed_order_fails_closed() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let error = calibrate_cubic_cfi_bijective_pebble_ordered(
            &base,
            &twists,
            &twists,
            vec![0; 40],
            natural_order(40),
            1,
            0,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CfiBijectivePebbleCalibrationError::Descriptive(
                DescriptiveError::DuplicateOrderElement(0)
            )
        ));
    }

    #[test]
    fn parity_is_recorded_without_promoting_an_outcome() {
        let base = k4();
        let even = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let odd =
            CfiTwistAssignment::new(&base, vec![true, false, false, false, false, false]).unwrap();
        let order = natural_order(40);
        let record = calibrate_cubic_cfi_bijective_pebble_ordered(
            &base,
            &even,
            &odd,
            order.clone(),
            order,
            1,
            0,
        )
        .unwrap();
        assert!(!record.left_odd_parity());
        assert!(record.right_odd_parity());
        assert_eq!(record.left_twist_count(), 0);
        assert_eq!(record.right_twist_count(), 1);
    }
}
