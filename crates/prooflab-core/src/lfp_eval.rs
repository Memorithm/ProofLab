//! Exact least-fixed-point evaluation for the PL-DC positive LFP substrate.
//!
//! This evaluator is a deterministic finite-model oracle. It starts from the
//! empty recursive relation and repeatedly applies the validated positive body
//! until the relation stabilizes. It does not prove a complexity-theoretic
//! characterization and it is intentionally optimized for correctness rather
//! than large state spaces.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    FiniteStructure, FoAtom, LfpAtom, LfpBody, LfpDefinition, LfpScopeError, LfpValidationError,
    OrderedFiniteStructure, Variable,
};

/// Materialized least fixed point of one finite LFP definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeastFixedPoint {
    tuples: Vec<Vec<u64>>,
    iterations: usize,
}

impl LeastFixedPoint {
    /// Return fixed-point tuples in canonical lexicographic order.
    #[must_use]
    pub fn tuples(&self) -> &[Vec<u64>] {
        &self.tuples
    }

    /// Return the number of operator applications, including the final stable pass.
    #[must_use]
    pub const fn iterations(&self) -> usize {
        self.iterations
    }

    /// Test exact membership of one tuple.
    #[must_use]
    pub fn contains(&self, tuple: &[u64]) -> bool {
        self.tuples
            .binary_search_by(|candidate| candidate.as_slice().cmp(tuple))
            .is_ok()
    }
}

/// Evaluate a positive LFP definition over an unordered finite structure.
///
/// # Errors
///
/// Returns [`LfpEvaluationError`] when syntax, scope, state-space materialization,
/// or exact evaluation fails.
pub fn evaluate_lfp_unordered(
    definition: &LfpDefinition,
    structure: &FiniteStructure,
) -> Result<LeastFixedPoint, LfpEvaluationError> {
    evaluate_lfp(definition, structure, None)
}

/// Evaluate a positive LFP definition over a finite structure with a distinguished order.
///
/// # Errors
///
/// Returns [`LfpEvaluationError`] when syntax, scope, state-space materialization,
/// or exact evaluation fails.
pub fn evaluate_lfp_ordered(
    definition: &LfpDefinition,
    structure: &OrderedFiniteStructure,
) -> Result<LeastFixedPoint, LfpEvaluationError> {
    evaluate_lfp(definition, structure.structure(), Some(structure))
}

fn evaluate_lfp(
    definition: &LfpDefinition,
    structure: &FiniteStructure,
    order: Option<&OrderedFiniteStructure>,
) -> Result<LeastFixedPoint, LfpEvaluationError> {
    definition
        .validate(structure.vocabulary(), order.is_some())
        .map_err(LfpEvaluationError::Definition)?;
    definition
        .validate_scope()
        .map_err(LfpEvaluationError::Scope)?;

    let state_space = checked_state_space(structure.domain_size(), definition.arity())?;
    ensure_tuple_buffer_addressable(definition.arity())?;

    let mut current = BTreeSet::new();
    let mut iterations = 0usize;

    loop {
        let mut next = BTreeSet::new();
        for_each_tuple(structure.domain_size(), definition.arity(), |candidate| {
            let mut bindings = BTreeMap::new();
            for (&variable, &value) in definition.parameters().iter().zip(candidate.iter()) {
                let previous = bindings.insert(variable, value);
                debug_assert!(previous.is_none());
            }

            if evaluate_body(definition.body(), structure, order, &current, &mut bindings)? {
                let _ = next.insert(candidate.to_vec());
            }
            Ok(())
        })?;

        iterations = iterations.saturating_add(1);
        if next == current {
            return Ok(LeastFixedPoint {
                tuples: current.into_iter().collect(),
                iterations,
            });
        }

        if !next.is_superset(&current) {
            return Err(LfpEvaluationError::MonotonicityInvariantViolated);
        }

        if iterations > state_space {
            return Err(LfpEvaluationError::ConvergenceInvariantViolated {
                state_space,
                iterations,
            });
        }

        current = next;
    }
}

fn checked_state_space(domain_size: u64, arity: usize) -> Result<usize, LfpEvaluationError> {
    let domain = usize::try_from(domain_size)
        .map_err(|_| LfpEvaluationError::StateSpaceNotAddressable { domain_size, arity })?;
    let mut total = 1usize;
    for _ in 0..arity {
        total = total
            .checked_mul(domain)
            .ok_or(LfpEvaluationError::StateSpaceNotAddressable { domain_size, arity })?;
    }
    Ok(total)
}

fn ensure_tuple_buffer_addressable(arity: usize) -> Result<(), LfpEvaluationError> {
    let mut tuple = Vec::<u64>::new();
    tuple
        .try_reserve_exact(arity)
        .map_err(|_| LfpEvaluationError::TupleBufferNotAddressable { arity })?;
    Ok(())
}

fn for_each_tuple<F>(domain_size: u64, arity: usize, mut visit: F) -> Result<(), LfpEvaluationError>
where
    F: FnMut(&[u64]) -> Result<(), LfpEvaluationError>,
{
    if arity == 0 {
        return visit(&[]);
    }

    let mut tuple = Vec::new();
    tuple
        .try_reserve_exact(arity)
        .map_err(|_| LfpEvaluationError::TupleBufferNotAddressable { arity })?;
    tuple.resize(arity, 0);

    loop {
        visit(&tuple)?;

        let mut position = arity;
        loop {
            if position == 0 {
                return Ok(());
            }
            position -= 1;
            tuple[position] = tuple[position].saturating_add(1);
            if tuple[position] < domain_size {
                break;
            }
            tuple[position] = 0;
        }
    }
}

fn evaluate_body(
    body: &LfpBody,
    structure: &FiniteStructure,
    order: Option<&OrderedFiniteStructure>,
    recursive: &BTreeSet<Vec<u64>>,
    bindings: &mut BTreeMap<Variable, u64>,
) -> Result<bool, LfpEvaluationError> {
    match body {
        LfpBody::True => Ok(true),
        LfpBody::False => Ok(false),
        LfpBody::Atom(atom) => evaluate_atom(atom, structure, order, recursive, bindings),
        LfpBody::Not(inner) => Ok(!evaluate_body(
            inner, structure, order, recursive, bindings,
        )?),
        LfpBody::And(parts) => {
            for part in parts {
                if !evaluate_body(part, structure, order, recursive, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        LfpBody::Or(parts) => {
            for part in parts {
                if evaluate_body(part, structure, order, recursive, bindings)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        LfpBody::Exists { variable, body } => {
            for value in 0..structure.domain_size() {
                let previous = bindings.insert(*variable, value);
                let result = evaluate_body(body, structure, order, recursive, bindings);
                restore_binding(bindings, *variable, previous);
                if result? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        LfpBody::ForAll { variable, body } => {
            for value in 0..structure.domain_size() {
                let previous = bindings.insert(*variable, value);
                let result = evaluate_body(body, structure, order, recursive, bindings);
                restore_binding(bindings, *variable, previous);
                if !result? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn evaluate_atom(
    atom: &LfpAtom,
    structure: &FiniteStructure,
    order: Option<&OrderedFiniteStructure>,
    recursive: &BTreeSet<Vec<u64>>,
    bindings: &BTreeMap<Variable, u64>,
) -> Result<bool, LfpEvaluationError> {
    match atom {
        LfpAtom::Recursive(args) => {
            let tuple = resolve_tuple(args, bindings)?;
            Ok(recursive.contains(&tuple))
        }
        LfpAtom::FirstOrder(atom) => match atom {
            FoAtom::Equal(left, right) => {
                Ok(resolve(*left, bindings)? == resolve(*right, bindings)?)
            }
            FoAtom::LessThan(left, right) => {
                let ordered = order.ok_or(LfpEvaluationError::OrderNotAvailable)?;
                Ok(ordered.less_than(resolve(*left, bindings)?, resolve(*right, bindings)?))
            }
            FoAtom::Relation { name, args } => {
                let relation = structure
                    .relation(name)
                    .ok_or_else(|| LfpEvaluationError::MissingRelation(name.clone()))?;
                let tuple = resolve_tuple(args, bindings)?;
                Ok(relation.contains(&tuple))
            }
        },
    }
}

fn resolve_tuple(
    variables: &[Variable],
    bindings: &BTreeMap<Variable, u64>,
) -> Result<Vec<u64>, LfpEvaluationError> {
    variables
        .iter()
        .map(|&variable| resolve(variable, bindings))
        .collect()
}

fn resolve(
    variable: Variable,
    bindings: &BTreeMap<Variable, u64>,
) -> Result<u64, LfpEvaluationError> {
    bindings
        .get(&variable)
        .copied()
        .ok_or(LfpEvaluationError::UnboundVariable(variable))
}

fn restore_binding(
    bindings: &mut BTreeMap<Variable, u64>,
    variable: Variable,
    previous: Option<u64>,
) {
    if let Some(value) = previous {
        let _ = bindings.insert(variable, value);
    } else {
        let _ = bindings.remove(&variable);
    }
}

/// Exact-evaluation failures for finite positive LFP definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LfpEvaluationError {
    /// The LFP definition is invalid for the supplied vocabulary/order mode.
    Definition(LfpValidationError),
    /// The first-order free-variable scope of the definition is invalid.
    Scope(LfpScopeError),
    /// The complete tuple state space cannot be addressed on this platform.
    StateSpaceNotAddressable { domain_size: u64, arity: usize },
    /// A tuple buffer of the declared recursive arity cannot be allocated.
    TupleBufferNotAddressable { arity: usize },
    /// Evaluation reached a free variable with no binding.
    UnboundVariable(Variable),
    /// Evaluation reached an order atom without an ordered structure.
    OrderNotAvailable,
    /// A relation disappeared after successful definition validation.
    MissingRelation(String),
    /// A supposedly positive operator removed tuples between iterations.
    MonotonicityInvariantViolated,
    /// A monotone finite iteration exceeded the strict-increase bound.
    ConvergenceInvariantViolated {
        state_space: usize,
        iterations: usize,
    },
}

impl fmt::Display for LfpEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Definition(error) => write!(formatter, "invalid LFP definition: {error}"),
            Self::Scope(error) => write!(formatter, "invalid LFP variable scope: {error}"),
            Self::StateSpaceNotAddressable { domain_size, arity } => write!(
                formatter,
                "LFP state space {domain_size}^{arity} is not addressable on this platform"
            ),
            Self::TupleBufferNotAddressable { arity } => {
                write!(formatter, "LFP tuple arity {arity} cannot be materialized")
            }
            Self::UnboundVariable(variable) => {
                write!(formatter, "unbound LFP first-order variable {}", variable.0)
            }
            Self::OrderNotAvailable => {
                formatter.write_str("order atom evaluated without an ordered structure")
            }
            Self::MissingRelation(name) => {
                write!(
                    formatter,
                    "validated relation {name} is missing from structure"
                )
            }
            Self::MonotonicityInvariantViolated => formatter.write_str(
                "validated positive LFP operator violated the monotonicity runtime invariant",
            ),
            Self::ConvergenceInvariantViolated {
                state_space,
                iterations,
            } => write!(
                formatter,
                "LFP iteration exceeded finite convergence bound: {iterations} applications for {state_space} possible tuples"
            ),
        }
    }
}

impl Error for LfpEvaluationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DescriptiveError, RelationInterpretation, RelationSymbol, Vocabulary};

    fn graph(edges: Vec<Vec<u64>>) -> Result<FiniteStructure, DescriptiveError> {
        let edge = RelationSymbol::new("E", 2)?;
        let vocabulary = Vocabulary::new(vec![edge.clone()])?;
        let relation = RelationInterpretation::new(edge, edges)?;
        FiniteStructure::new(3, vocabulary, vec![relation])
    }

    fn reachability_definition() -> LfpDefinition {
        let x = Variable(0);
        let y = Variable(1);
        let z = Variable(2);
        let body = LfpBody::Or(vec![
            LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Equal(x, y))),
            LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Relation {
                name: "E".into(),
                args: vec![x, y],
            })),
            LfpBody::Exists {
                variable: z,
                body: Box::new(LfpBody::And(vec![
                    LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Relation {
                        name: "E".into(),
                        args: vec![x, z],
                    })),
                    LfpBody::Atom(LfpAtom::Recursive(vec![z, y])),
                ])),
            },
        ]);
        LfpDefinition::new(vec![x, y], body).unwrap()
    }

    #[test]
    fn reachability_converges_to_exact_transitive_reflexive_closure() {
        let structure = graph(vec![vec![0, 1], vec![1, 2]]).unwrap();
        let fixed = evaluate_lfp_unordered(&reachability_definition(), &structure).unwrap();

        assert_eq!(
            fixed.tuples(),
            &[
                vec![0, 0],
                vec![0, 1],
                vec![0, 2],
                vec![1, 1],
                vec![1, 2],
                vec![2, 2],
            ]
        );
        assert_eq!(fixed.iterations(), 3);
        assert!(fixed.contains(&[0, 2]));
        assert!(!fixed.contains(&[2, 0]));
    }

    #[test]
    fn distinguished_order_is_used_only_on_ordered_path() {
        let x = Variable(0);
        let y = Variable(1);
        let definition = LfpDefinition::new(
            vec![x],
            LfpBody::ForAll {
                variable: y,
                body: Box::new(LfpBody::Not(Box::new(LfpBody::Atom(LfpAtom::FirstOrder(
                    FoAtom::LessThan(y, x),
                ))))),
            },
        )
        .unwrap();
        let ordered = OrderedFiniteStructure::new(graph(vec![]).unwrap(), vec![2, 0, 1]).unwrap();

        let fixed = evaluate_lfp_ordered(&definition, &ordered).unwrap();
        assert_eq!(fixed.tuples(), &[vec![2]]);
        assert!(matches!(
            evaluate_lfp_unordered(&definition, ordered.structure()),
            Err(LfpEvaluationError::Definition(
                LfpValidationError::FirstOrder(_)
            ))
        ));
    }

    #[test]
    fn free_auxiliary_variables_are_rejected_before_iteration() {
        let x = Variable(0);
        let z = Variable(2);
        let definition = LfpDefinition::new(
            vec![x],
            LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Equal(x, z))),
        )
        .unwrap();

        assert!(matches!(
            evaluate_lfp_unordered(&definition, &graph(vec![]).unwrap()),
            Err(LfpEvaluationError::Scope(_))
        ));
    }

    #[test]
    fn zero_arity_fixed_point_is_supported() {
        let definition = LfpDefinition::new(vec![], LfpBody::True).unwrap();
        let fixed = evaluate_lfp_unordered(&definition, &graph(vec![]).unwrap()).unwrap();

        assert_eq!(fixed.tuples(), &[Vec::<u64>::new()]);
        assert_eq!(fixed.iterations(), 2);
    }

    #[test]
    fn unaddressable_state_space_fails_before_enumeration() {
        let arity = usize::BITS as usize + 1;
        let parameters = (0..arity)
            .map(|index| Variable(u64::try_from(index).unwrap()))
            .collect();
        let definition = LfpDefinition::new(parameters, LfpBody::False).unwrap();

        assert!(matches!(
            evaluate_lfp_unordered(&definition, &graph(vec![]).unwrap()),
            Err(LfpEvaluationError::StateSpaceNotAddressable { .. })
        ));
    }
}
