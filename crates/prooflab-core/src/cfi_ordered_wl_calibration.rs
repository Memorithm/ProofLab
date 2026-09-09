//! Explicit-order CFI-to-`k`-WL calibration records for PL-DC.
//!
//! This module exists to prevent a methodological shortcut: an outcome for one
//! chosen pair of distinguished orders is not evidence that a CFI phenomenon is
//! robust to adding arbitrary total orders. Every record therefore binds the
//! exact ordered-structure identities that were compared.

use crate::{
    CfiBaseGraph, CfiTwistAssignment, CfiWlCalibrationError, OrderedFiniteStructure,
    OrderedFiniteStructureId, compare_oblivious_wl_ordered, cubic_cfi_as_relational,
};

/// Finite computational evidence for one exact pair of ordered CFI structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedCfiWlCalibration {
    dimension: usize,
    left_structure: OrderedFiniteStructureId,
    right_structure: OrderedFiniteStructureId,
    distinguished: bool,
    refinement_applications: usize,
    stable_color_classes: usize,
}

impl OrderedCfiWlCalibration {
    /// `k` used by the coordinate-wise WL comparison.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Content identity of the exact left structure including its order.
    #[must_use]
    pub const fn left_structure(&self) -> OrderedFiniteStructureId {
        self.left_structure
    }

    /// Content identity of the exact right structure including its order.
    #[must_use]
    pub const fn right_structure(&self) -> OrderedFiniteStructureId {
        self.right_structure
    }

    /// Observed result for this exact pair of distinguished orders only.
    #[must_use]
    pub const fn distinguished(&self) -> bool {
        self.distinguished
    }

    /// Number of WL refinement applications, including the stable pass.
    #[must_use]
    pub const fn refinement_applications(&self) -> usize {
        self.refinement_applications
    }

    /// Number of stable joint color classes for the exact ordered pair.
    #[must_use]
    pub const fn stable_color_classes(&self) -> usize {
        self.stable_color_classes
    }
}

/// Compare two CFI twist assignments after attaching exact caller-supplied
/// total orders to their expanded carriers.
///
/// The returned result is finite calibration evidence for these orders only.
/// It must not be interpreted as an order-robust lower bound.
///
/// # Errors
///
/// Fails closed on invalid CFI construction, invalid order permutations,
/// relational translation failure, or an invalid/unaddressable `k`-WL run.
pub fn calibrate_cubic_cfi_wl_ordered(
    base: &CfiBaseGraph,
    left_twists: &CfiTwistAssignment,
    right_twists: &CfiTwistAssignment,
    left_order: Vec<u64>,
    right_order: Vec<u64>,
    dimension: usize,
) -> Result<OrderedCfiWlCalibration, CfiWlCalibrationError> {
    let left_graph = crate::CubicCfiGraph::new(base, left_twists)?;
    let right_graph = crate::CubicCfiGraph::new(base, right_twists)?;
    let left = cubic_cfi_as_relational(&left_graph)?;
    let right = cubic_cfi_as_relational(&right_graph)?;
    let left = OrderedFiniteStructure::new(left, left_order)
        .map_err(CfiWlCalibrationError::Descriptive)?;
    let right = OrderedFiniteStructure::new(right, right_order)
        .map_err(CfiWlCalibrationError::Descriptive)?;
    let comparison = compare_oblivious_wl_ordered(&left, &right, dimension)?;

    Ok(OrderedCfiWlCalibration {
        dimension,
        left_structure: left.id(),
        right_structure: right.id(),
        distinguished: comparison.distinguished(),
        refinement_applications: comparison.refinement_applications(),
        stable_color_classes: comparison.stable_color_classes(),
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
    fn identical_structure_and_order_are_not_distinguished() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let order = natural_order(40);
        let record =
            calibrate_cubic_cfi_wl_ordered(&base, &twists, &twists, order.clone(), order, 2)
                .unwrap();
        assert!(!record.distinguished());
        assert_eq!(record.left_structure(), record.right_structure());
    }

    #[test]
    fn order_identity_is_part_of_the_recorded_input() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let left_order = natural_order(40);
        let mut right_order = left_order.clone();
        right_order.swap(0, 1);
        let record =
            calibrate_cubic_cfi_wl_ordered(&base, &twists, &twists, left_order, right_order, 2)
                .unwrap();
        assert_ne!(record.left_structure(), record.right_structure());
    }

    #[test]
    fn malformed_order_is_rejected() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let error = calibrate_cubic_cfi_wl_ordered(
            &base,
            &twists,
            &twists,
            vec![0; 40],
            natural_order(40),
            2,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CfiWlCalibrationError::Descriptive(DescriptiveError::DuplicateOrderElement(0))
        ));
    }
}
