//! Finite CFI-to-`k`-WL calibration records for PL-DC.
//!
//! Results from this module are finite computational evidence only. They do not
//! establish a lower bound, non-isomorphism, or any separation of complexity
//! classes, and they are never promoted to `PROVED` without the formal-kernel
//! path owned by ProofLab.

use core::fmt;
use std::error::Error;

use crate::{
    CfiBaseGraph, CfiError, CfiTwistAssignment, DescriptiveError, FiniteStructureId,
    ObliviousWlError, cubic_cfi_as_relational, compare_oblivious_wl_unordered,
};

/// Deterministic finite calibration record for one pair of cubic CFI instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiWlCalibration {
    dimension: usize,
    left_structure: FiniteStructureId,
    right_structure: FiniteStructureId,
    left_twist_count: usize,
    right_twist_count: usize,
    left_odd_parity: bool,
    right_odd_parity: bool,
    distinguished: bool,
    refinement_applications: usize,
    stable_color_classes: usize,
}

impl CfiWlCalibration {
    #[must_use]
    pub const fn dimension(&self) -> usize { self.dimension }
    #[must_use]
    pub const fn left_structure(&self) -> FiniteStructureId { self.left_structure }
    #[must_use]
    pub const fn right_structure(&self) -> FiniteStructureId { self.right_structure }
    #[must_use]
    pub const fn left_twist_count(&self) -> usize { self.left_twist_count }
    #[must_use]
    pub const fn right_twist_count(&self) -> usize { self.right_twist_count }
    #[must_use]
    pub const fn left_odd_parity(&self) -> bool { self.left_odd_parity }
    #[must_use]
    pub const fn right_odd_parity(&self) -> bool { self.right_odd_parity }
    #[must_use]
    pub const fn distinguished(&self) -> bool { self.distinguished }
    #[must_use]
    pub const fn refinement_applications(&self) -> usize { self.refinement_applications }
    #[must_use]
    pub const fn stable_color_classes(&self) -> usize { self.stable_color_classes }
}

/// Error while constructing or comparing finite CFI calibration instances.
#[derive(Debug)]
pub enum CfiWlCalibrationError {
    Cfi(CfiError),
    Descriptive(DescriptiveError),
    Wl(ObliviousWlError),
}

impl fmt::Display for CfiWlCalibrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cfi(error) => write!(f, "CFI construction failed: {error}"),
            Self::Descriptive(error) => write!(f, "CFI relational bridge failed: {error}"),
            Self::Wl(error) => write!(f, "CFI k-WL calibration failed: {error}"),
        }
    }
}

impl Error for CfiWlCalibrationError {}

impl From<CfiError> for CfiWlCalibrationError {
    fn from(value: CfiError) -> Self { Self::Cfi(value) }
}
impl From<DescriptiveError> for CfiWlCalibrationError {
    fn from(value: DescriptiveError) -> Self { Self::Descriptive(value) }
}
impl From<ObliviousWlError> for CfiWlCalibrationError {
    fn from(value: ObliviousWlError) -> Self { Self::Wl(value) }
}

/// Compare two twist assignments on the same cubic base graph with exact
/// coordinate-wise `k`-WL and return an auditable finite calibration record.
///
/// # Errors
///
/// Fails closed on invalid CFI input, relational translation failure, or an
/// invalid/unaddressable `k`-WL comparison.
pub fn calibrate_cubic_cfi_wl(
    base: &CfiBaseGraph,
    left_twists: &CfiTwistAssignment,
    right_twists: &CfiTwistAssignment,
    dimension: usize,
) -> Result<CfiWlCalibration, CfiWlCalibrationError> {
    let left_graph = crate::CubicCfiGraph::new(base, left_twists)?;
    let right_graph = crate::CubicCfiGraph::new(base, right_twists)?;
    let left = cubic_cfi_as_relational(&left_graph)?;
    let right = cubic_cfi_as_relational(&right_graph)?;
    let comparison = compare_oblivious_wl_unordered(&left, &right, dimension)?;

    Ok(CfiWlCalibration {
        dimension,
        left_structure: left.id(),
        right_structure: right.id(),
        left_twist_count: left_twists.twist_count(),
        right_twist_count: right_twists.twist_count(),
        left_odd_parity: left_twists.odd_parity(),
        right_odd_parity: right_twists.odd_parity(),
        distinguished: comparison.distinguished(),
        refinement_applications: comparison.refinement_applications(),
        stable_color_classes: comparison.stable_color_classes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k4() -> CfiBaseGraph {
        CfiBaseGraph::new(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]).unwrap()
    }

    #[test]
    fn self_comparison_is_not_distinguished() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let record = calibrate_cubic_cfi_wl(&base, &twists, &twists, 2).unwrap();
        assert!(!record.distinguished());
        assert_eq!(record.left_structure(), record.right_structure());
        assert_eq!(record.dimension(), 2);
    }

    #[test]
    fn pair_record_is_deterministic_without_assuming_the_outcome() {
        let base = k4();
        let even = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let odd = CfiTwistAssignment::new(&base, vec![true, false, false, false, false, false]).unwrap();
        let first = calibrate_cubic_cfi_wl(&base, &even, &odd, 2).unwrap();
        let second = calibrate_cubic_cfi_wl(&base, &even, &odd, 2).unwrap();
        assert_eq!(first, second);
        assert_ne!(first.left_structure(), first.right_structure());
        assert!(!first.left_odd_parity());
        assert!(first.right_odd_parity());
    }
}
