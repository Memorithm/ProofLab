//! Exact finite Ehrenfeucht–Fraïssé game solver for the PL-DC programme.
//!
//! The solver answers whether Duplicator has a winning strategy for a fixed
//! number of rounds on two explicit finite relational structures. Positions are
//! accepted only when the pebbled correspondence is a partial isomorphism.
//! This is an executable finite-game oracle, not a formal proof of the EF theorem.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{FiniteStructure, OrderedFiniteStructure, Vocabulary};

/// Deterministic result of one finite EF game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EfGameResult {
    duplicator_wins: bool,
    rounds: u32,
    states_explored: u64,
}

impl EfGameResult {
    /// Whether Duplicator has a winning strategy for the requested number of rounds.
    #[must_use]
    pub const fn duplicator_wins(self) -> bool {
        self.duplicator_wins
    }

    /// Requested number of game rounds.
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

/// Solve the exact `rounds`-round EF game on unordered structures.
///
/// # Errors
///
/// Returns [`EfGameError`] when the structures use different vocabularies, a
/// relation arity cannot be materialized, or the state counter overflows.
pub fn solve_ef_unordered(
    left: &FiniteStructure,
    right: &FiniteStructure,
    rounds: u32,
) -> Result<EfGameResult, EfGameError> {
    solve(left, right, None, None, rounds)
}

/// Solve the exact `rounds`-round EF game with distinguished orders.
///
/// # Errors
///
/// Returns [`EfGameError`] when the structures use different vocabularies, a
/// relation arity cannot be materialized, or the state counter overflows.
pub fn solve_ef_ordered(
    left: &OrderedFiniteStructure,
    right: &OrderedFiniteStructure,
    rounds: u32,
) -> Result<EfGameResult, EfGameError> {
    solve(
        left.structure(),
        right.structure(),
        Some(left),
        Some(right),
        rounds,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GameState {
    remaining: u32,
    pebbles: Vec<(u64, u64)>,
}

struct Solver<'a> {
    left: &'a FiniteStructure,
    right: &'a FiniteStructure,
    left_order: Option<&'a OrderedFiniteStructure>,
    right_order: Option<&'a OrderedFiniteStructure>,
    memo: BTreeMap<GameState, bool>,
    states_explored: u64,
}

fn solve(
    left: &FiniteStructure,
    right: &FiniteStructure,
    left_order: Option<&OrderedFiniteStructure>,
    right_order: Option<&OrderedFiniteStructure>,
    rounds: u32,
) -> Result<EfGameResult, EfGameError> {
    if left.vocabulary() != right.vocabulary() {
        return Err(EfGameError::VocabularyMismatch);
    }

    let mut solver = Solver {
        left,
        right,
        left_order,
        right_order,
        memo: BTreeMap::new(),
        states_explored: 0,
    };
    let duplicator_wins = solver.wins(&GameState {
        remaining: rounds,
        pebbles: Vec::new(),
    })?;

    Ok(EfGameResult {
        duplicator_wins,
        rounds,
        states_explored: solver.states_explored,
    })
}

impl Solver<'_> {
    fn wins(&mut self, state: &GameState) -> Result<bool, EfGameError> {
        if let Some(&cached) = self.memo.get(state) {
            return Ok(cached);
        }

        self.states_explored = self
            .states_explored
            .checked_add(1)
            .ok_or(EfGameError::StateCounterOverflow)?;

        if !is_partial_isomorphism(
            self.left,
            self.right,
            self.left_order,
            self.right_order,
            &state.pebbles,
        )? {
            let _ = self.memo.insert(state.clone(), false);
            return Ok(false);
        }

        if state.remaining == 0 {
            let _ = self.memo.insert(state.clone(), true);
            return Ok(true);
        }

        let next_remaining = state.remaining - 1;

        for left_element in 0..self.left.domain_size() {
            let mut has_reply = false;
            for right_element in 0..self.right.domain_size() {
                let mut child = state.pebbles.clone();
                child.push((left_element, right_element));
                if self.wins(&GameState {
                    remaining: next_remaining,
                    pebbles: child,
                })? {
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
                let mut child = state.pebbles.clone();
                child.push((left_element, right_element));
                if self.wins(&GameState {
                    remaining: next_remaining,
                    pebbles: child,
                })? {
                    has_reply = true;
                    break;
                }
            }
            if !has_reply {
                let _ = self.memo.insert(state.clone(), false);
                return Ok(false);
            }
        }

        let _ = self.memo.insert(state.clone(), true);
        Ok(true)
    }
}

fn is_partial_isomorphism(
    left: &FiniteStructure,
    right: &FiniteStructure,
    left_order: Option<&OrderedFiniteStructure>,
    right_order: Option<&OrderedFiniteStructure>,
    pebbles: &[(u64, u64)],
) -> Result<bool, EfGameError> {
    if !preserves_equality(pebbles) {
        return Ok(false);
    }

    if !preserves_relations(left, right, left.vocabulary(), pebbles)? {
        return Ok(false);
    }

    match (left_order, right_order) {
        (Some(left_order), Some(right_order)) => {
            Ok(preserves_order(left_order, right_order, pebbles))
        }
        (None, None) => Ok(true),
        _ => Err(EfGameError::OrderModeMismatch),
    }
}

fn preserves_equality(pebbles: &[(u64, u64)]) -> bool {
    for (left_index, &(left_a, right_a)) in pebbles.iter().enumerate() {
        for &(left_b, right_b) in &pebbles[left_index..] {
            if (left_a == left_b) != (right_a == right_b) {
                return false;
            }
        }
    }
    true
}

fn preserves_order(
    left: &OrderedFiniteStructure,
    right: &OrderedFiniteStructure,
    pebbles: &[(u64, u64)],
) -> bool {
    for &(left_a, right_a) in pebbles {
        for &(left_b, right_b) in pebbles {
            if left.less_than(left_a, left_b) != right.less_than(right_a, right_b) {
                return false;
            }
        }
    }
    true
}

fn preserves_relations(
    left: &FiniteStructure,
    right: &FiniteStructure,
    vocabulary: &Vocabulary,
    pebbles: &[(u64, u64)],
) -> Result<bool, EfGameError> {
    for symbol in vocabulary.relations() {
        let arity =
            usize::try_from(symbol.arity()).map_err(|_| EfGameError::ArityNotAddressable {
                relation: symbol.name().to_owned(),
                arity: symbol.arity(),
            })?;
        let left_relation = left
            .relation(symbol.name())
            .ok_or_else(|| EfGameError::MissingRelation(symbol.name().to_owned()))?;
        let right_relation = right
            .relation(symbol.name())
            .ok_or_else(|| EfGameError::MissingRelation(symbol.name().to_owned()))?;

        if arity == 0 {
            if left_relation.contains(&[]) != right_relation.contains(&[]) {
                return Ok(false);
            }
            continue;
        }

        if pebbles.is_empty() {
            continue;
        }

        let mut indices = Vec::new();
        indices
            .try_reserve_exact(arity)
            .map_err(|_| EfGameError::TupleIndexBufferNotAddressable { arity })?;
        indices.resize(arity, 0usize);

        loop {
            let mut left_tuple = Vec::new();
            let mut right_tuple = Vec::new();
            left_tuple
                .try_reserve_exact(arity)
                .map_err(|_| EfGameError::TupleBufferNotAddressable { arity })?;
            right_tuple
                .try_reserve_exact(arity)
                .map_err(|_| EfGameError::TupleBufferNotAddressable { arity })?;
            for &index in &indices {
                let (left_element, right_element) = pebbles[index];
                left_tuple.push(left_element);
                right_tuple.push(right_element);
            }

            if left_relation.contains(&left_tuple) != right_relation.contains(&right_tuple) {
                return Ok(false);
            }

            let mut position = arity;
            loop {
                if position == 0 {
                    break;
                }
                position -= 1;
                indices[position] += 1;
                if indices[position] < pebbles.len() {
                    break;
                }
                indices[position] = 0;
            }
            if position == 0 && indices[0] == 0 {
                break;
            }
        }
    }

    Ok(true)
}

/// Fail-closed errors for exact finite EF games.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EfGameError {
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
    /// The deterministic state instrumentation counter overflowed.
    StateCounterOverflow,
}

impl fmt::Display for EfGameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VocabularyMismatch => {
                formatter.write_str("EF structures use different vocabularies")
            }
            Self::OrderModeMismatch => {
                formatter.write_str("EF solver mixed ordered and unordered modes")
            }
            Self::ArityNotAddressable { relation, arity } => write!(
                formatter,
                "relation {relation} has EF-unaddressable arity {arity}"
            ),
            Self::MissingRelation(name) => {
                write!(
                    formatter,
                    "validated relation {name} is missing from EF structure"
                )
            }
            Self::TupleIndexBufferNotAddressable { arity } => {
                write!(
                    formatter,
                    "EF tuple-index arity {arity} cannot be materialized"
                )
            }
            Self::TupleBufferNotAddressable { arity } => {
                write!(
                    formatter,
                    "EF relation tuple arity {arity} cannot be materialized"
                )
            }
            Self::StateCounterOverflow => formatter.write_str("EF state counter overflowed"),
        }
    }
}

impl Error for EfGameError {}

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
    fn isomorphic_empty_structures_are_indistinguishable_for_tested_rounds() {
        let left = empty_structure(3);
        let right = empty_structure(3);

        for rounds in 0..=4 {
            assert!(
                solve_ef_unordered(&left, &right, rounds)
                    .unwrap()
                    .duplicator_wins()
            );
        }
    }

    #[test]
    fn equality_distinguishes_cardinality_one_from_two_in_two_rounds() {
        let one = empty_structure(1);
        let two = empty_structure(2);

        assert!(solve_ef_unordered(&one, &two, 1).unwrap().duplicator_wins());
        assert!(!solve_ef_unordered(&one, &two, 2).unwrap().duplicator_wins());
    }

    #[test]
    fn unary_relation_difference_is_seen_in_one_round() {
        let marked = unary_structure(2, vec![0]).unwrap();
        let unmarked = unary_structure(2, vec![]).unwrap();

        assert!(
            !solve_ef_unordered(&marked, &unmarked, 1)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn nullary_relation_difference_is_visible_at_zero_rounds() {
        let truth = nullary_structure(true).unwrap();
        let falsity = nullary_structure(false).unwrap();

        assert!(
            !solve_ef_unordered(&truth, &falsity, 0)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn ordered_solver_preserves_distinguished_order() {
        let left = OrderedFiniteStructure::new(empty_structure(2), vec![0, 1]).unwrap();
        let right = OrderedFiniteStructure::new(empty_structure(3), vec![2, 0, 1]).unwrap();

        assert!(
            solve_ef_ordered(&left, &right, 1)
                .unwrap()
                .duplicator_wins()
        );
        assert!(
            !solve_ef_ordered(&left, &right, 3)
                .unwrap()
                .duplicator_wins()
        );
    }

    #[test]
    fn vocabulary_mismatch_fails_closed() {
        let empty = empty_structure(1);
        let unary = unary_structure(1, vec![]).unwrap();

        assert_eq!(
            solve_ef_unordered(&empty, &unary, 1),
            Err(EfGameError::VocabularyMismatch)
        );
    }

    #[test]
    fn instrumentation_is_deterministic() {
        let left = empty_structure(2);
        let right = empty_structure(2);
        let first = solve_ef_unordered(&left, &right, 2).unwrap();
        let second = solve_ef_unordered(&left, &right, 2).unwrap();

        assert_eq!(first, second);
        assert!(first.states_explored() > 0);
    }
}
