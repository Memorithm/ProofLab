//! Shared partial-isomorphism checks for finite model-comparison games.
//!
//! Ehrenfeucht–Fraïssé, ordinary pebble, and later bijective pebble games all
//! rely on the same semantic invariant: the currently paired elements must
//! induce a partial isomorphism. Keeping that invariant here prevents the game
//! solvers from silently drifting apart.

use crate::{FiniteStructure, OrderedFiniteStructure, Vocabulary};

pub(crate) fn ensure_compatible(
    left: &FiniteStructure,
    right: &FiniteStructure,
) -> Result<(), PartialIsoError> {
    if left.vocabulary() == right.vocabulary() {
        Ok(())
    } else {
        Err(PartialIsoError::VocabularyMismatch)
    }
}

pub(crate) fn is_partial_isomorphism(
    left: &FiniteStructure,
    right: &FiniteStructure,
    left_order: Option<&OrderedFiniteStructure>,
    right_order: Option<&OrderedFiniteStructure>,
    pebbles: &[(u64, u64)],
) -> Result<bool, PartialIsoError> {
    ensure_compatible(left, right)?;

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
        _ => Err(PartialIsoError::OrderModeMismatch),
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
) -> Result<bool, PartialIsoError> {
    for symbol in vocabulary.relations() {
        let arity = usize::try_from(symbol.arity()).map_err(|_| {
            PartialIsoError::ArityNotAddressable {
                relation: symbol.name().to_owned(),
                arity: symbol.arity(),
            }
        })?;
        let left_relation = left
            .relation(symbol.name())
            .ok_or_else(|| PartialIsoError::MissingRelation(symbol.name().to_owned()))?;
        let right_relation = right
            .relation(symbol.name())
            .ok_or_else(|| PartialIsoError::MissingRelation(symbol.name().to_owned()))?;

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
            .map_err(|_| PartialIsoError::TupleIndexBufferNotAddressable { arity })?;
        indices.resize(arity, 0usize);

        loop {
            let mut left_tuple = Vec::new();
            let mut right_tuple = Vec::new();
            left_tuple
                .try_reserve_exact(arity)
                .map_err(|_| PartialIsoError::TupleBufferNotAddressable { arity })?;
            right_tuple
                .try_reserve_exact(arity)
                .map_err(|_| PartialIsoError::TupleBufferNotAddressable { arity })?;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PartialIsoError {
    VocabularyMismatch,
    OrderModeMismatch,
    ArityNotAddressable { relation: String, arity: u64 },
    MissingRelation(String),
    TupleIndexBufferNotAddressable { arity: usize },
    TupleBufferNotAddressable { arity: usize },
}
