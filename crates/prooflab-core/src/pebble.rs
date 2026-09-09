//! Exact ordinary finite `k`-pebble game solver for the PL-DC programme.
//!
//! Pebble pairs are reusable: on each round Spoiler chooses a pair, chooses one
//! of the two structures, and places that pebble on an element. Duplicator then
//! places the matching pebble in the other structure. The active pebble mapping
//! must remain a partial isomorphism. This is the ordinary pebble game, not the
//! bijective/counting variant.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::partial_iso::{PartialIsoError, ensure_compatible, is_partial_isomorphism};
use crate::{FiniteStructure, OrderedFiniteStructure};

/// Deterministic result of one finite ordinary pebble game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PebbleGameResult {
    duplicator_wins: bool,
    pebble_pairs: usize,
    rounds: u32,
    states_explored: u64,
}

impl PebbleGameResult {
    /// Whether Duplicator has a winning strategy.
    #[must_use]
    pub const fn duplicator_wins(self) -> bool {
        self.duplicator_wins
    }

    /// Number of reusable pebble pairs available in the game.
    #[must_use]
    pub const fn pebble_pairs(self) -> usize {
        self.pebble_pairs
    }

    /// Requested number of rounds.
    #[must_use]
    pub const fn rounds(self) -> u32 {
        self.rounds
    }

    /// Number of previously unseen game states evaluated by the exact solver.
    #[must_use]
    pub const fn states_explored(self) -> u64 {
        self.states_explored
    }
}

/// Solve the exact ordinary `rounds`-round `k`-pebble game on unordered structures.
///
/// # Errors
///
/// Returns [`PebbleGameError`] when the structures are incompatible, finite game
/// buffers cannot be represented, or deterministic instrumentation overflows.
pub fn solve_pebble_unordered(
    left: &FiniteStructure,
    right: &FiniteStructure,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<PebbleGameResult, PebbleGameError> {
    solve(left, right, None, None, pebble_pairs, rounds)
}

/// Solve the exact ordinary `rounds`-round `k`-pebble game with distinguished orders.
///
/// # Errors
///
/// Returns [`PebbleGameError`] when the structures are incompatible, finite game
/// buffers cannot be represented, or deterministic instrumentation overflows.
pub fn solve_pebble_ordered(
    left: &OrderedFiniteStructure,
    right: &OrderedFiniteStructure,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<PebbleGameResult, PebbleGameError> {
    solve(
        left.structure(),
        right.structure(),
        Some(left),
        Some(right),
        pebble_pairs,
        rounds,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PebbleState {
    remaining: u32,
    slots: Vec<Option<(u64, u64)>>,
}

struct Solver<'a> {
    left: &'a FiniteStructure,
    right: &'a FiniteStructure,
    left_order: Option<&'a OrderedFiniteStructure>,
    right_order: Option<&'a OrderedFiniteStructure>,
    memo: BTreeMap<PebbleState, bool>,
    states_explored: u64,
}

fn solve(
    left: &FiniteStructure,
    right: &FiniteStructure,
    left_order: Option<&OrderedFiniteStructure>,
    right_order: Option<&OrderedFiniteStructure>,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<PebbleGameResult, PebbleGameError> {
    ensure_compatible(left, right).map_err(PebbleGameError::from)?;

    let mut slots = Vec::new();
    slots
        .try_reserve_exact(pebble_pairs)
        .map_err(|_| PebbleGameError::PebbleSlotsNotAddressable { pebble_pairs })?;
    slots.resize(pebble_pairs, None);

    let mut solver = Solver {
        left,
        right,
        left_order,
        right_order,
        memo: BTreeMap::new(),
        states_explored: 0,
    };
    let duplicator_wins = solver.wins(&PebbleState {
        remaining: rounds,
        slots,
    })?;

    Ok(PebbleGameResult {
        duplicator_wins,
        pebble_pairs,
        rounds,
        states_explored: solver.states_explored,
    })
}

impl Solver<'_> {
    fn wins(&mut self, state: &PebbleState) -> Result<bool, PebbleGameError> {
        if let Some(&cached) = self.memo.get(state) {
            return Ok(cached);
        }

        self.states_explored = self
            .states_explored
            .checked_add(1)
            .ok_or(PebbleGameError::StateCounterOverflow)?;

        let active = active_pairs(&state.slots)?;
        if !is_partial_isomorphism(
            self.left,
            self.right,
            self.left_order,
            self.right_order,
            &active,
        )
        .map_err(PebbleGameError::from)?
        {
            let _ = self.memo.insert(state.clone(), false);
            return Ok(false);
        }

        if state.remaining == 0 || state.slots.is_empty() {
            let _ = self.memo.insert(state.clone(), true);
            return Ok(true);
        }

        let next_remaining = state.remaining - 1;

        for slot in 0..state.slots.len() {
            for left_element in 0..self.left.domain_size() {
                let mut has_reply = false;
                for right_element in 0..self.right.domain_size() {
                    let child =
                        replace_slot(state, slot, (left_element, right_element), next_remaining);
                    if self.wins(&child)? {
                        has_reply = true;
                        break;
                    }
                }
                if !has_reply {
                    let _ = self.memo.insert(state.clone(), false);
                    return Ok(false);
                }
            }

            for right_element in 0..self.right.domain_size() {
                let mut has_reply = false;
                for left_element in 0..self.left.domain_size() {
                    let child =
                        replace_slot(state, slot, (left_element, right_element), next_remaining);
                    if self.wins(&child)? {
                        has_reply = true;
                        break;
                    }
                }
                if !has_reply {
                    let _ = self.memo.insert(state.clone(), false);
                    return Ok(false);
                }
            }
        }

        let _ = self.memo.insert(state.clone(), true);
        Ok(true)
    }
}

fn replace_slot(state: &PebbleState, slot: usize, pair: (u64, u64), remaining: u32) -> PebbleState {
    let mut slots = state.slots.clone();
    slots[slot] = Some(pair);
    PebbleState { remaining, slots }
}

fn active_pairs(slots: &[Option<(u64, u64)>]) -> Result<Vec<(u64, u64)>, PebbleGameError> {
    let mut active = Vec::new();
    active.try_reserve_exact(slots.len()).map_err(|_| {
        PebbleGameError::ActivePebblesNotAddressable {
            pebble_pairs: slots.len(),
        }
    })?;
    active.extend(slots.iter().flatten().copied());
    Ok(active)
}

/// Fail-closed errors for exact finite ordinary pebble games.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PebbleGameError {
    /// Both structures must interpret exactly the same vocabulary.
    VocabularyMismatch,
    /// Ordered and unordered evaluation modes must not be mixed internally.
    OrderModeMismatch,
    /// A relation arity cannot be represented on this platform.
    ArityNotAddressable { relation: String, arity: u64 },
    /// A validated structure is unexpectedly missing a relation interpretation.
    MissingRelation(String),
    /// The tuple-index buffer cannot be materialized for a relation arity.
    TupleIndexBufferNotAddressable { arity: usize },
    /// A relation tuple buffer cannot be materialized for a relation arity.
    TupleBufferNotAddressable { arity: usize },
    /// The requested pebble-slot vector cannot be represented on this platform.
    PebbleSlotsNotAddressable { pebble_pairs: usize },
    /// The active-pebble projection cannot be represented on this platform.
    ActivePebblesNotAddressable { pebble_pairs: usize },
    /// The deterministic state instrumentation counter overflowed.
    StateCounterOverflow,
}

impl From<PartialIsoError> for PebbleGameError {
    fn from(error: PartialIsoError) -> Self {
        match error {
            PartialIsoError::VocabularyMismatch => Self::VocabularyMismatch,
            PartialIsoError::OrderModeMismatch => Self::OrderModeMismatch,
            PartialIsoError::ArityNotAddressable { relation, arity } => {
                Self::ArityNotAddressable { relation, arity }
            }
            PartialIsoError::MissingRelation(name) => Self::MissingRelation(name),
            PartialIsoError::TupleIndexBufferNotAddressable { arity } => {
                Self::TupleIndexBufferNotAddressable { arity }
            }
            PartialIsoError::TupleBufferNotAddressable { arity } => {
                Self::TupleBufferNotAddressable { arity }
            }
        }
    }
}

impl fmt::Display for PebbleGameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VocabularyMismatch => {
                formatter.write_str("pebble-game structures use different vocabularies")
            }
            Self::OrderModeMismatch => {
                formatter.write_str("pebble-game solver mixed ordered and unordered modes")
            }
            Self::ArityNotAddressable { relation, arity } => write!(
                formatter,
                "relation {relation} has pebble-game-unaddressable arity {arity}"
            ),
            Self::MissingRelation(name) => write!(
                formatter,
                "validated relation {name} is missing from pebble-game structure"
            ),
            Self::TupleIndexBufferNotAddressable { arity } => write!(
                formatter,
                "pebble-game tuple-index arity {arity} cannot be materialized"
            ),
            Self::TupleBufferNotAddressable { arity } => write!(
                formatter,
                "pebble-game relation tuple arity {arity} cannot be materialized"
            ),
            Self::PebbleSlotsNotAddressable { pebble_pairs } => write!(
                formatter,
                "{pebble_pairs} pebble pairs cannot be represented on this platform"
            ),
            Self::ActivePebblesNotAddressable { pebble_pairs } => write!(
                formatter,
                "active projection for {pebble_pairs} pebble pairs cannot be represented"
            ),
            Self::StateCounterOverflow => {
                formatter.write_str("pebble-game state counter overflowed")
            }
        }
    }
}

impl Error for PebbleGameError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DescriptiveError, RelationInterpretation, RelationSymbol, Vocabulary};

    fn empty_structure(size: u64) -> FiniteStructure {
        FiniteStructure::new(size, Vocabulary::new(vec![]).unwrap(), vec![]).unwrap()
    }

    fn unary_structure(size: u64, marked: Vec<u64>) -> Result<FiniteStructure, DescriptiveError> {
        let mark = RelationSymbol::new("P", 1)?;
        let vocabulary = Vocabulary::new(vec![mark.clone()])?;
        let tuples = marked.into_iter().map(|value| vec![value]).collect();
        let relation = RelationInterpretation::new(mark, tuples)?;
        FiniteStructure::new(size, vocabulary, vec![relation])
    }

    fn nullary_structure(value: bool) -> Result<FiniteStructure, DescriptiveError> {
        let flag = RelationSymbol::new("Q", 0)?;
        let vocabulary = Vocabulary::new(vec![flag.clone()])?;
        let tuples = if value { vec![vec![]] } else { vec![] };
        let relation = RelationInterpretation::new(flag, tuples)?;
        FiniteStructure::new(1, vocabulary, vec![relation])
    }

    #[test]
    fn one_pebble_cannot_count_one_against_two() {
        let one = empty_structure(1);
        let two = empty_structure(2);

        let result = solve_pebble_unordered(&one, &two, 1, 5).unwrap();
        assert!(result.duplicator_wins());
        assert_eq!(result.pebble_pairs(), 1);
        assert_eq!(result.rounds(), 5);
    }

    #[test]
    fn two_pebbles_distinguish_one_against_two_in_two_rounds() {
        let one = empty_structure(1);
        let two = empty_structure(2);

        assert!(
            !solve_pebble_unordered(&one, &two, 2, 2)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn unary_relation_difference_is_seen_with_one_pebble() {
        let marked = unary_structure(2, vec![0]).unwrap();
        let unmarked = unary_structure(2, vec![]).unwrap();

        assert!(
            !solve_pebble_unordered(&marked, &unmarked, 1, 1)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn zero_pebbles_still_observe_nullary_relations() {
        let truth = nullary_structure(true).unwrap();
        let falsity = nullary_structure(false).unwrap();

        assert!(
            !solve_pebble_unordered(&truth, &falsity, 0, 10)
                .unwrap()
                .duplicator_wins()
        );
        assert!(
            solve_pebble_unordered(&truth, &truth, 0, 10)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn isomorphic_distinguished_orders_are_indistinguishable() {
        let left = OrderedFiniteStructure::new(empty_structure(3), vec![0, 1, 2]).unwrap();
        let right = OrderedFiniteStructure::new(empty_structure(3), vec![2, 0, 1]).unwrap();

        assert!(
            solve_pebble_ordered(&left, &right, 3, 4)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn vocabulary_mismatch_fails_closed() {
        let empty = empty_structure(1);
        let unary = unary_structure(1, vec![]).unwrap();

        assert_eq!(
            solve_pebble_unordered(&empty, &unary, 1, 1),
            Err(PebbleGameError::VocabularyMismatch)
        );
    }

    #[test]
    fn impossible_pebble_vector_fails_closed() {
        let left = empty_structure(1);
        let right = empty_structure(1);

        assert_eq!(
            solve_pebble_unordered(&left, &right, usize::MAX, 0),
            Err(PebbleGameError::PebbleSlotsNotAddressable {
                pebble_pairs: usize::MAX,
            })
        );
    }

    #[test]
    fn instrumentation_is_deterministic() {
        let left = empty_structure(2);
        let right = empty_structure(2);
        let first = solve_pebble_unordered(&left, &right, 2, 2).unwrap();
        let second = solve_pebble_unordered(&left, &right, 2, 2).unwrap();

        assert_eq!(first, second);
        assert!(first.states_explored() > 0);
    }
}
