//! Bounded Ehrenfeucht-Fraisse calibration for parity over pure equality.
//!
//! For a caller-supplied finite round bound `r`, this module compares one even
//! and one odd finite carrier whose cardinalities are both at least `r`. The
//! exact EF solver then checks whether Duplicator wins the `r`-round game.
//!
//! A Duplicator win here is finite calibration evidence only. No bounded game,
//! finite collection of carrier sizes, or successful run of this helper proves
//! that parity is not first-order definable. The all-quantifier-rank theorem is
//! a separate formal obligation.

use crate::{EfGameError, EfGameResult, FiniteStructure, Vocabulary, solve_ef_unordered};

/// Exact result of one bounded parity-vs-equality EF calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParityEfCalibration {
    rounds: u32,
    even_size: u64,
    odd_size: u64,
    game: EfGameResult,
}

impl ParityEfCalibration {
    /// EF round bound checked by the exact solver.
    #[must_use]
    pub const fn rounds(self) -> u32 {
        self.rounds
    }

    /// Cardinality of the even pure-equality structure.
    #[must_use]
    pub const fn even_size(self) -> u64 {
        self.even_size
    }

    /// Cardinality of the odd pure-equality structure.
    #[must_use]
    pub const fn odd_size(self) -> u64 {
        self.odd_size
    }

    /// Exact finite EF result, including deterministic state instrumentation.
    #[must_use]
    pub const fn game(self) -> EfGameResult {
        self.game
    }

    /// Whether Duplicator survived the requested finite round bound.
    #[must_use]
    pub const fn duplicator_wins(self) -> bool {
        self.game.duplicator_wins()
    }
}

fn calibration_sizes(rounds: u32) -> (u64, u64) {
    let required = u64::from(rounds).max(1);
    let even_size = if required.is_multiple_of(2) {
        required
    } else {
        required + 1
    };
    (even_size, even_size + 1)
}

fn pure_equality_structure(size: u64) -> FiniteStructure {
    FiniteStructure::new(
        size,
        Vocabulary::new(vec![]).expect("empty vocabulary is valid"),
        vec![],
    )
    .expect("calibration size is always nonzero")
}

/// Run one exact bounded EF calibration between an even and odd pure-equality
/// carrier.
///
/// The chosen cardinalities are deterministic: the even carrier is the least
/// positive even integer at least `rounds`, and the odd carrier is the next
/// integer. Thus both structures have at least as many elements as game rounds.
/// No additional search cutoff is imposed by this adapter; computational cost
/// is exactly that of the underlying exact EF solver for the requested bound.
///
/// # Errors
///
/// Propagates exact EF solver failures, including instrumentation overflow.
pub fn calibrate_parity_pure_equality(rounds: u32) -> Result<ParityEfCalibration, EfGameError> {
    let (even_size, odd_size) = calibration_sizes(rounds);
    let even = pure_equality_structure(even_size);
    let odd = pure_equality_structure(odd_size);
    let game = solve_ef_unordered(&even, &odd, rounds)?;

    Ok(ParityEfCalibration {
        rounds,
        even_size,
        odd_size,
        game,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_sizes_preserve_opposite_parity_and_round_capacity() {
        for rounds in 0..=8 {
            let (even, odd) = calibration_sizes(rounds);
            assert_eq!(even % 2, 0);
            assert_eq!(odd % 2, 1);
            assert!(even >= u64::from(rounds));
            assert!(odd >= u64::from(rounds));
            assert_eq!(odd, even + 1);
        }
    }

    #[test]
    fn duplicator_wins_small_bounded_parity_controls() {
        for rounds in 0..=3 {
            let calibration = calibrate_parity_pure_equality(rounds).unwrap();
            assert!(calibration.duplicator_wins());
            assert_eq!(calibration.rounds(), rounds);
            assert_eq!(calibration.game().rounds(), rounds);
            assert!(calibration.game().states_explored() > 0);
        }
    }

    #[test]
    fn instrumentation_is_deterministic() {
        let first = calibrate_parity_pure_equality(3).unwrap();
        let second = calibrate_parity_pure_equality(3).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn one_more_round_can_expose_a_too_small_pair_outside_the_adapter_contract() {
        let two = pure_equality_structure(2);
        let three = pure_equality_structure(3);
        assert!(
            solve_ef_unordered(&two, &three, 2)
                .unwrap()
                .duplicator_wins()
        );
        assert!(
            !solve_ef_unordered(&two, &three, 3)
                .unwrap()
                .duplicator_wins()
        );
    }
}
