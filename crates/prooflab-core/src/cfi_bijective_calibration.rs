//! Finite CFI-to-bijective-pebble calibration records for PL-DC.
//!
//! Results from this module are finite computational evidence only. They do not
//! establish a lower bound, non-isomorphism, or any separation of complexity
//! classes, and they are never promoted to `PROVED` without the formal-kernel
//! path owned by `ProofLab`.

use core::fmt;
use std::error::Error;

use crate::{
    BijectivePebbleGameError, CfiBaseGraph, CfiError, CfiTwistAssignment, DescriptiveError,
    FiniteStructureId, cubic_cfi_as_relational, solve_bijective_pebble_unordered,
};

/// Deterministic finite calibration record for one pair of cubic CFI instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiBijectivePebbleCalibration {
    pebble_pairs: usize,
    rounds: u32,
    left_structure: FiniteStructureId,
    right_structure: FiniteStructureId,
    left_twist_count: usize,
    right_twist_count: usize,
    left_odd_parity: bool,
    right_odd_parity: bool,
    duplicator_wins: bool,
    states_explored: u64,
    bijections_considered: u64,
}

impl CfiBijectivePebbleCalibration {
    /// Number of reusable pebble pairs in the exact finite game.
    #[must_use]
    pub const fn pebble_pairs(&self) -> usize {
        self.pebble_pairs
    }

    /// Number of requested game rounds.
    #[must_use]
    pub const fn rounds(&self) -> u32 {
        self.rounds
    }

    /// Content identity of the exact left relational CFI structure.
    #[must_use]
    pub const fn left_structure(&self) -> FiniteStructureId {
        self.left_structure
    }

    /// Content identity of the exact right relational CFI structure.
    #[must_use]
    pub const fn right_structure(&self) -> FiniteStructureId {
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

    /// Observed exact finite game outcome.
    #[must_use]
    pub const fn duplicator_wins(&self) -> bool {
        self.duplicator_wins
    }

    /// Number of previously unseen game states explored by the solver.
    #[must_use]
    pub const fn states_explored(&self) -> u64 {
        self.states_explored
    }

    /// Number of candidate bijections considered by the solver.
    #[must_use]
    pub const fn bijections_considered(&self) -> u64 {
        self.bijections_considered
    }
}

/// Error while constructing or solving one finite CFI bijective-pebble calibration.
#[derive(Debug)]
pub enum CfiBijectivePebbleCalibrationError {
    Cfi(CfiError),
    Descriptive(DescriptiveError),
    Game(BijectivePebbleGameError),
}

impl fmt::Display for CfiBijectivePebbleCalibrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cfi(error) => write!(formatter, "CFI construction failed: {error}"),
            Self::Descriptive(error) => write!(formatter, "CFI relational bridge failed: {error}"),
            Self::Game(error) => write!(
                formatter,
                "CFI bijective-pebble calibration failed: {error}"
            ),
        }
    }
}

impl Error for CfiBijectivePebbleCalibrationError {}

impl From<CfiError> for CfiBijectivePebbleCalibrationError {
    fn from(value: CfiError) -> Self {
        Self::Cfi(value)
    }
}

impl From<DescriptiveError> for CfiBijectivePebbleCalibrationError {
    fn from(value: DescriptiveError) -> Self {
        Self::Descriptive(value)
    }
}

impl From<BijectivePebbleGameError> for CfiBijectivePebbleCalibrationError {
    fn from(value: BijectivePebbleGameError) -> Self {
        Self::Game(value)
    }
}

/// Compare two cubic CFI twist assignments with the exact finite bijective
/// pebble-game solver and return an auditable calibration record.
///
/// The result is finite computational evidence for the supplied base graph,
/// twist assignments, pebble count, and round bound only. It is not an
/// asymptotic lower bound and must not be interpreted as one.
///
/// # Errors
///
/// Fails closed on invalid CFI construction, relational translation failure,
/// or a bijective-pebble solver error.
pub fn calibrate_cubic_cfi_bijective_pebble(
    base: &CfiBaseGraph,
    left_twists: &CfiTwistAssignment,
    right_twists: &CfiTwistAssignment,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<CfiBijectivePebbleCalibration, CfiBijectivePebbleCalibrationError> {
    let left_graph = crate::CubicCfiGraph::new(base, left_twists)?;
    let right_graph = crate::CubicCfiGraph::new(base, right_twists)?;
    let left = cubic_cfi_as_relational(&left_graph)?;
    let right = cubic_cfi_as_relational(&right_graph)?;
    let result = solve_bijective_pebble_unordered(&left, &right, pebble_pairs, rounds)?;

    Ok(CfiBijectivePebbleCalibration {
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

    fn k4() -> CfiBaseGraph {
        CfiBaseGraph::new(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]).unwrap()
    }

    #[test]
    fn self_comparison_is_a_deterministic_finite_control() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let first = calibrate_cubic_cfi_bijective_pebble(&base, &twists, &twists, 1, 1).unwrap();
        let second = calibrate_cubic_cfi_bijective_pebble(&base, &twists, &twists, 1, 1).unwrap();
        assert_eq!(first, second);
        assert!(first.duplicator_wins());
        assert_eq!(first.left_structure(), first.right_structure());
        assert_eq!(first.pebble_pairs(), 1);
        assert_eq!(first.rounds(), 1);
    }

    #[test]
    fn parity_and_structure_identity_are_recorded_without_assuming_an_outcome() {
        let base = k4();
        let even = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let odd =
            CfiTwistAssignment::new(&base, vec![true, false, false, false, false, false]).unwrap();
        let record = calibrate_cubic_cfi_bijective_pebble(&base, &even, &odd, 1, 0).unwrap();
        assert_ne!(record.left_structure(), record.right_structure());
        assert_eq!(record.left_twist_count(), 0);
        assert_eq!(record.right_twist_count(), 1);
        assert!(!record.left_odd_parity());
        assert!(record.right_odd_parity());
    }
}
