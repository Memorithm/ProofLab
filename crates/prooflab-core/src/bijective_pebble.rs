//! Exact finite bijective `k`-pebble game solver for the PL-DC programme.
//!
//! In each round Spoiler chooses a reusable pebble pair. Duplicator must then
//! choose a bijection from the left carrier to the right carrier that agrees
//! with every other active pebble pair. Only after seeing that bijection does
//! Spoiler choose the left element for the selected pebble. This is the
//! bijective/counting pebble variant, deliberately separate from the ordinary
//! pebble game.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::partial_iso::{PartialIsoError, ensure_compatible, is_partial_isomorphism};
use crate::{FiniteStructure, OrderedFiniteStructure};

/// Deterministic result of one finite bijective pebble game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BijectivePebbleGameResult {
    duplicator_wins: bool,
    pebble_pairs: usize,
    rounds: u32,
    states_explored: u64,
    bijections_considered: u64,
}

impl BijectivePebbleGameResult {
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

    /// Number of candidate bijections tested across all explored game states.
    #[must_use]
    pub const fn bijections_considered(self) -> u64 {
        self.bijections_considered
    }
}

/// Solve the exact finite bijective pebble game on unordered structures.
///
/// # Errors
///
/// Returns [`BijectivePebbleGameError`] when the structures are incompatible,
/// required finite buffers cannot be represented, or deterministic
/// instrumentation overflows.
pub fn solve_bijective_pebble_unordered(
    left: &FiniteStructure,
    right: &FiniteStructure,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<BijectivePebbleGameResult, BijectivePebbleGameError> {
    solve(left, right, None, None, pebble_pairs, rounds)
}

/// Solve the exact finite bijective pebble game with distinguished orders.
///
/// # Errors
///
/// Returns [`BijectivePebbleGameError`] when the structures are incompatible,
/// required finite buffers cannot be represented, or deterministic
/// instrumentation overflows.
pub fn solve_bijective_pebble_ordered(
    left: &OrderedFiniteStructure,
    right: &OrderedFiniteStructure,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<BijectivePebbleGameResult, BijectivePebbleGameError> {
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
struct BijectiveState {
    remaining: u32,
    slots: Vec<Option<(u64, u64)>>,
}

struct Solver<'a> {
    left: &'a FiniteStructure,
    right: &'a FiniteStructure,
    left_order: Option<&'a OrderedFiniteStructure>,
    right_order: Option<&'a OrderedFiniteStructure>,
    memo: BTreeMap<BijectiveState, bool>,
    states_explored: u64,
    bijections_considered: u64,
}

fn solve(
    left: &FiniteStructure,
    right: &FiniteStructure,
    left_order: Option<&OrderedFiniteStructure>,
    right_order: Option<&OrderedFiniteStructure>,
    pebble_pairs: usize,
    rounds: u32,
) -> Result<BijectivePebbleGameResult, BijectivePebbleGameError> {
    ensure_compatible(left, right).map_err(BijectivePebbleGameError::from)?;

    if left.domain_size() != right.domain_size() {
        return Ok(BijectivePebbleGameResult {
            duplicator_wins: false,
            pebble_pairs,
            rounds,
            states_explored: 0,
            bijections_considered: 0,
        });
    }

    let mut slots = Vec::new();
    slots
        .try_reserve_exact(pebble_pairs)
        .map_err(|_| BijectivePebbleGameError::PebbleSlotsNotAddressable { pebble_pairs })?;
    slots.resize(pebble_pairs, None);

    let mut solver = Solver {
        left,
        right,
        left_order,
        right_order,
        memo: BTreeMap::new(),
        states_explored: 0,
        bijections_considered: 0,
    };
    let duplicator_wins = solver.wins(&BijectiveState {
        remaining: rounds,
        slots,
    })?;

    Ok(BijectivePebbleGameResult {
        duplicator_wins,
        pebble_pairs,
        rounds,
        states_explored: solver.states_explored,
        bijections_considered: solver.bijections_considered,
    })
}

impl Solver<'_> {
    fn wins(&mut self, state: &BijectiveState) -> Result<bool, BijectivePebbleGameError> {
        if let Some(&cached) = self.memo.get(state) {
            return Ok(cached);
        }

        self.states_explored = self
            .states_explored
            .checked_add(1)
            .ok_or(BijectivePebbleGameError::StateCounterOverflow)?;

        let active = active_pairs(&state.slots)?;
        if !is_partial_isomorphism(
            self.left,
            self.right,
            self.left_order,
            self.right_order,
            &active,
        )
        .map_err(BijectivePebbleGameError::from)?
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
            if !self.has_winning_bijection(state, slot, next_remaining)? {
                let _ = self.memo.insert(state.clone(), false);
                return Ok(false);
            }
        }

        let _ = self.memo.insert(state.clone(), true);
        Ok(true)
    }

    fn has_winning_bijection(
        &mut self,
        state: &BijectiveState,
        moving_slot: usize,
        next_remaining: u32,
    ) -> Result<bool, BijectivePebbleGameError> {
        let domain_size = self.left.domain_size();
        let domain = usize::try_from(domain_size)
            .map_err(|_| BijectivePebbleGameError::DomainNotAddressable { domain_size })?;

        let mut mapping = Vec::new();
        mapping
            .try_reserve_exact(domain)
            .map_err(|_| BijectivePebbleGameError::BijectionBufferNotAddressable { domain_size })?;
        mapping.resize(domain, None);

        let mut used_right = Vec::new();
        used_right
            .try_reserve_exact(domain)
            .map_err(|_| BijectivePebbleGameError::BijectionBufferNotAddressable { domain_size })?;
        used_right.resize(domain, false);

        for (slot, placement) in state.slots.iter().enumerate() {
            if slot == moving_slot {
                continue;
            }
            let Some((left_element, right_element)) = placement else {
                continue;
            };
            let left_index = usize::try_from(*left_element)
                .map_err(|_| BijectivePebbleGameError::DomainNotAddressable { domain_size })?;
            let right_index = usize::try_from(*right_element)
                .map_err(|_| BijectivePebbleGameError::DomainNotAddressable { domain_size })?;

            match mapping[left_index] {
                Some(existing) if existing != *right_element => return Ok(false),
                Some(_) => {}
                None => {
                    if used_right[right_index] {
                        return Ok(false);
                    }
                    mapping[left_index] = Some(*right_element);
                    used_right[right_index] = true;
                }
            }
        }

        let mut free_left = Vec::new();
        let mut free_right = Vec::new();
        free_left
            .try_reserve_exact(domain)
            .map_err(|_| BijectivePebbleGameError::BijectionBufferNotAddressable { domain_size })?;
        free_right
            .try_reserve_exact(domain)
            .map_err(|_| BijectivePebbleGameError::BijectionBufferNotAddressable { domain_size })?;

        for value in 0..domain_size {
            let index = usize::try_from(value)
                .map_err(|_| BijectivePebbleGameError::DomainNotAddressable { domain_size })?;
            if mapping[index].is_none() {
                free_left.push(value);
            }
            if !used_right[index] {
                free_right.push(value);
            }
        }

        debug_assert_eq!(free_left.len(), free_right.len());

        loop {
            for (&left_element, &right_element) in free_left.iter().zip(&free_right) {
                let left_index = usize::try_from(left_element)
                    .map_err(|_| BijectivePebbleGameError::DomainNotAddressable { domain_size })?;
                mapping[left_index] = Some(right_element);
            }

            self.bijections_considered = self
                .bijections_considered
                .checked_add(1)
                .ok_or(BijectivePebbleGameError::BijectionCounterOverflow)?;

            let mut survives_every_spoiler_choice = true;
            for left_element in 0..domain_size {
                let left_index = usize::try_from(left_element)
                    .map_err(|_| BijectivePebbleGameError::DomainNotAddressable { domain_size })?;
                let right_element = mapping[left_index]
                    .ok_or(BijectivePebbleGameError::IncompleteBijectionInvariant)?;
                let child = replace_slot(
                    state,
                    moving_slot,
                    (left_element, right_element),
                    next_remaining,
                );
                if !self.wins(&child)? {
                    survives_every_spoiler_choice = false;
                    break;
                }
            }

            if survives_every_spoiler_choice {
                return Ok(true);
            }

            if !next_permutation(&mut free_right) {
                break;
            }
        }

        Ok(false)
    }
}

fn replace_slot(
    state: &BijectiveState,
    slot: usize,
    pair: (u64, u64),
    remaining: u32,
) -> BijectiveState {
    let mut slots = state.slots.clone();
    slots[slot] = Some(pair);
    BijectiveState { remaining, slots }
}

fn active_pairs(slots: &[Option<(u64, u64)>]) -> Result<Vec<(u64, u64)>, BijectivePebbleGameError> {
    let mut active = Vec::new();
    active.try_reserve_exact(slots.len()).map_err(|_| {
        BijectivePebbleGameError::ActivePebblesNotAddressable {
            pebble_pairs: slots.len(),
        }
    })?;
    active.extend(slots.iter().flatten().copied());
    Ok(active)
}

fn next_permutation(values: &mut [u64]) -> bool {
    if values.len() < 2 {
        return false;
    }

    let mut pivot = values.len() - 2;
    while values[pivot] >= values[pivot + 1] {
        if pivot == 0 {
            values.reverse();
            return false;
        }
        pivot -= 1;
    }

    let mut successor = values.len() - 1;
    while values[successor] <= values[pivot] {
        successor -= 1;
    }

    values.swap(pivot, successor);
    values[pivot + 1..].reverse();
    true
}

/// Fail-closed errors for exact finite bijective pebble games.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BijectivePebbleGameError {
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
    /// The finite carrier cannot be indexed by this platform.
    DomainNotAddressable { domain_size: u64 },
    /// A complete candidate bijection buffer cannot be represented.
    BijectionBufferNotAddressable { domain_size: u64 },
    /// Internal candidate construction failed to produce a total bijection.
    IncompleteBijectionInvariant,
    /// The deterministic state instrumentation counter overflowed.
    StateCounterOverflow,
    /// The deterministic bijection instrumentation counter overflowed.
    BijectionCounterOverflow,
}

impl From<PartialIsoError> for BijectivePebbleGameError {
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

impl fmt::Display for BijectivePebbleGameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VocabularyMismatch => {
                formatter.write_str("bijective-pebble structures use different vocabularies")
            }
            Self::OrderModeMismatch => {
                formatter.write_str("bijective-pebble solver mixed ordered and unordered modes")
            }
            Self::ArityNotAddressable { relation, arity } => write!(
                formatter,
                "relation {relation} has bijective-pebble-unaddressable arity {arity}"
            ),
            Self::MissingRelation(name) => write!(
                formatter,
                "validated relation {name} is missing from bijective-pebble structure"
            ),
            Self::TupleIndexBufferNotAddressable { arity } => write!(
                formatter,
                "bijective-pebble tuple-index arity {arity} cannot be materialized"
            ),
            Self::TupleBufferNotAddressable { arity } => write!(
                formatter,
                "bijective-pebble relation tuple arity {arity} cannot be materialized"
            ),
            Self::PebbleSlotsNotAddressable { pebble_pairs } => write!(
                formatter,
                "{pebble_pairs} bijective pebble pairs cannot be represented"
            ),
            Self::ActivePebblesNotAddressable { pebble_pairs } => write!(
                formatter,
                "active projection for {pebble_pairs} bijective pebble pairs cannot be represented"
            ),
            Self::DomainNotAddressable { domain_size } => write!(
                formatter,
                "bijective-pebble carrier of size {domain_size} cannot be indexed"
            ),
            Self::BijectionBufferNotAddressable { domain_size } => write!(
                formatter,
                "bijection buffer for carrier size {domain_size} cannot be represented"
            ),
            Self::IncompleteBijectionInvariant => {
                formatter.write_str("candidate bijection was unexpectedly incomplete")
            }
            Self::StateCounterOverflow => {
                formatter.write_str("bijective-pebble state counter overflowed")
            }
            Self::BijectionCounterOverflow => {
                formatter.write_str("bijective-pebble bijection counter overflowed")
            }
        }
    }
}

impl Error for BijectivePebbleGameError {}

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
    fn unequal_cardinality_loses_before_any_round() {
        let one = empty_structure(1);
        let two = empty_structure(2);

        let result = solve_bijective_pebble_unordered(&one, &two, 1, 0).unwrap();
        assert!(!result.duplicator_wins());
        assert_eq!(result.states_explored(), 0);
        assert_eq!(result.bijections_considered(), 0);
    }

    #[test]
    fn equal_empty_structures_are_indistinguishable() {
        let left = empty_structure(3);
        let right = empty_structure(3);

        let result = solve_bijective_pebble_unordered(&left, &right, 2, 3).unwrap();
        assert!(result.duplicator_wins());
        assert_eq!(result.pebble_pairs(), 2);
        assert_eq!(result.rounds(), 3);
        assert!(result.bijections_considered() > 0);
    }

    #[test]
    fn bijection_move_detects_unary_count_difference() {
        let one_marked = unary_structure(2, vec![0]).unwrap();
        let none_marked = unary_structure(2, vec![]).unwrap();

        assert!(
            !solve_bijective_pebble_unordered(&one_marked, &none_marked, 1, 1)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn relabelled_unary_structure_has_a_winning_bijection() {
        let left = unary_structure(2, vec![0]).unwrap();
        let right = unary_structure(2, vec![1]).unwrap();

        assert!(
            solve_bijective_pebble_unordered(&left, &right, 1, 2)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn zero_pebbles_still_observe_nullary_relations() {
        let truth = nullary_structure(true).unwrap();
        let falsity = nullary_structure(false).unwrap();

        assert!(
            !solve_bijective_pebble_unordered(&truth, &falsity, 0, 4)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn isomorphic_distinguished_orders_are_indistinguishable() {
        let left = OrderedFiniteStructure::new(empty_structure(3), vec![0, 1, 2]).unwrap();
        let right = OrderedFiniteStructure::new(empty_structure(3), vec![2, 0, 1]).unwrap();

        assert!(
            solve_bijective_pebble_ordered(&left, &right, 2, 3)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn vocabulary_mismatch_fails_closed() {
        let empty = empty_structure(1);
        let unary = unary_structure(1, vec![]).unwrap();

        assert_eq!(
            solve_bijective_pebble_unordered(&empty, &unary, 1, 1),
            Err(BijectivePebbleGameError::VocabularyMismatch)
        );
    }

    #[test]
    fn impossible_pebble_vector_fails_closed() {
        let left = empty_structure(1);
        let right = empty_structure(1);

        assert_eq!(
            solve_bijective_pebble_unordered(&left, &right, usize::MAX, 0),
            Err(BijectivePebbleGameError::PebbleSlotsNotAddressable {
                pebble_pairs: usize::MAX,
            })
        );
    }

    #[test]
    fn instrumentation_is_deterministic() {
        let left = empty_structure(2);
        let right = empty_structure(2);
        let first = solve_bijective_pebble_unordered(&left, &right, 2, 2).unwrap();
        let second = solve_bijective_pebble_unordered(&left, &right, 2, 2).unwrap();

        assert_eq!(first, second);
        assert!(first.states_explored() > 0);
        assert!(first.bijections_considered() > 0);
    }
}
