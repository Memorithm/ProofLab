//! Exact finite-model evaluation for the PL-DC first-order syntax.
//!
//! This evaluator is a reference semantic oracle, not a proof procedure. It
//! evaluates a finite FO formula exactly on an explicitly materialized finite
//! structure and keeps ordered and unordered evaluation paths separate.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{
    FiniteStructure, FoAtom, FoFormula, FoValidationError, OrderedFiniteStructure, Variable,
};

/// A finite assignment of first-order variables to domain elements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoAssignment {
    bindings: BTreeMap<Variable, u64>,
}

impl FoAssignment {
    /// Construct an assignment from explicit bindings.
    ///
    /// Domain bounds are checked when the assignment is evaluated against a
    /// concrete structure.
    ///
    /// # Errors
    ///
    /// Returns [`FoEvaluationError::DuplicateBinding`] when the same syntactic
    /// variable occurs more than once.
    pub fn new(bindings: Vec<(Variable, u64)>) -> Result<Self, FoEvaluationError> {
        let mut normalized = BTreeMap::new();
        for (variable, value) in bindings {
            if normalized.insert(variable, value).is_some() {
                return Err(FoEvaluationError::DuplicateBinding(variable));
            }
        }
        Ok(Self {
            bindings: normalized,
        })
    }

    /// Return the assigned value of `variable`, if present.
    #[must_use]
    pub fn get(&self, variable: Variable) -> Option<u64> {
        self.bindings.get(&variable).copied()
    }

    /// Return normalized bindings in variable order.
    #[must_use]
    pub const fn bindings(&self) -> &BTreeMap<Variable, u64> {
        &self.bindings
    }
}

/// Evaluate an FO formula on an unordered finite structure.
///
/// # Errors
///
/// Returns [`FoEvaluationError`] when formula validation fails, an assignment
/// references an out-of-domain element, or a free variable is unbound.
pub fn evaluate_unordered(
    formula: &FoFormula,
    structure: &FiniteStructure,
    assignment: &FoAssignment,
) -> Result<bool, FoEvaluationError> {
    evaluate(formula, structure, None, assignment)
}

/// Evaluate an FO formula on a finite structure with its distinguished order.
///
/// # Errors
///
/// Returns [`FoEvaluationError`] when formula validation fails, an assignment
/// references an out-of-domain element, or a free variable is unbound.
pub fn evaluate_ordered(
    formula: &FoFormula,
    structure: &OrderedFiniteStructure,
    assignment: &FoAssignment,
) -> Result<bool, FoEvaluationError> {
    evaluate(formula, structure.structure(), Some(structure), assignment)
}

fn evaluate(
    formula: &FoFormula,
    structure: &FiniteStructure,
    order: Option<&OrderedFiniteStructure>,
    assignment: &FoAssignment,
) -> Result<bool, FoEvaluationError> {
    formula
        .validate(structure.vocabulary(), order.is_some())
        .map_err(FoEvaluationError::Formula)?;
    validate_assignment_domain(assignment, structure.domain_size())?;

    let mut bindings = assignment.bindings.clone();
    evaluate_inner(formula, structure, order, &mut bindings)
}

fn validate_assignment_domain(
    assignment: &FoAssignment,
    domain_size: u64,
) -> Result<(), FoEvaluationError> {
    for (&variable, &value) in &assignment.bindings {
        if value >= domain_size {
            return Err(FoEvaluationError::AssignmentOutOfDomain {
                variable,
                value,
                domain_size,
            });
        }
    }
    Ok(())
}

fn evaluate_inner(
    formula: &FoFormula,
    structure: &FiniteStructure,
    order: Option<&OrderedFiniteStructure>,
    bindings: &mut BTreeMap<Variable, u64>,
) -> Result<bool, FoEvaluationError> {
    match formula {
        FoFormula::True => Ok(true),
        FoFormula::False => Ok(false),
        FoFormula::Atom(atom) => evaluate_atom(atom, structure, order, bindings),
        FoFormula::Not(body) => Ok(!evaluate_inner(body, structure, order, bindings)?),
        FoFormula::And(parts) => {
            for part in parts {
                if !evaluate_inner(part, structure, order, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        FoFormula::Or(parts) => {
            for part in parts {
                if evaluate_inner(part, structure, order, bindings)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        FoFormula::Exists { variable, body } => {
            for value in 0..structure.domain_size() {
                let previous = bindings.insert(*variable, value);
                let result = evaluate_inner(body, structure, order, bindings);
                restore_binding(bindings, *variable, previous);
                if result? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        FoFormula::ForAll { variable, body } => {
            for value in 0..structure.domain_size() {
                let previous = bindings.insert(*variable, value);
                let result = evaluate_inner(body, structure, order, bindings);
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
    atom: &FoAtom,
    structure: &FiniteStructure,
    order: Option<&OrderedFiniteStructure>,
    bindings: &BTreeMap<Variable, u64>,
) -> Result<bool, FoEvaluationError> {
    match atom {
        FoAtom::Equal(left, right) => Ok(resolve(*left, bindings)? == resolve(*right, bindings)?),
        FoAtom::LessThan(left, right) => {
            let ordered = order.ok_or(FoEvaluationError::OrderNotAvailable)?;
            Ok(ordered.less_than(resolve(*left, bindings)?, resolve(*right, bindings)?))
        }
        FoAtom::Relation { name, args } => {
            let relation = structure
                .relation(name)
                .ok_or_else(|| FoEvaluationError::MissingRelation(name.clone()))?;
            let mut tuple = Vec::with_capacity(args.len());
            for &variable in args {
                tuple.push(resolve(variable, bindings)?);
            }
            Ok(relation.contains(&tuple))
        }
    }
}

fn resolve(
    variable: Variable,
    bindings: &BTreeMap<Variable, u64>,
) -> Result<u64, FoEvaluationError> {
    bindings
        .get(&variable)
        .copied()
        .ok_or(FoEvaluationError::UnboundVariable(variable))
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

/// Exact-evaluation failures for PL-DC FO formulas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoEvaluationError {
    /// The formula is not valid for the supplied vocabulary/order mode.
    Formula(FoValidationError),
    /// An explicit assignment repeats a syntactic variable.
    DuplicateBinding(Variable),
    /// An explicit assignment references an element outside the domain.
    AssignmentOutOfDomain {
        variable: Variable,
        value: u64,
        domain_size: u64,
    },
    /// Evaluation reached a free variable with no assignment.
    UnboundVariable(Variable),
    /// Evaluation reached an order atom without an ordered structure.
    OrderNotAvailable,
    /// A relation disappeared after successful formula validation.
    MissingRelation(String),
}

impl fmt::Display for FoEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Formula(error) => write!(formatter, "invalid FO formula: {error}"),
            Self::DuplicateBinding(variable) => {
                write!(formatter, "duplicate binding for variable {}", variable.0)
            }
            Self::AssignmentOutOfDomain {
                variable,
                value,
                domain_size,
            } => write!(
                formatter,
                "variable {} is assigned {value}, outside domain size {domain_size}",
                variable.0
            ),
            Self::UnboundVariable(variable) => {
                write!(formatter, "unbound FO variable {}", variable.0)
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
        }
    }
}

impl Error for FoEvaluationError {}

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

    #[test]
    fn existential_formula_uses_exact_relation_semantics() {
        let x = Variable(0);
        let y = Variable(1);
        let has_edge = FoFormula::Exists {
            variable: x,
            body: Box::new(FoFormula::Exists {
                variable: y,
                body: Box::new(FoFormula::Atom(FoAtom::Relation {
                    name: "E".into(),
                    args: vec![x, y],
                })),
            }),
        };

        assert_eq!(
            evaluate_unordered(
                &has_edge,
                &graph(vec![vec![0, 1]]).unwrap(),
                &FoAssignment::default()
            ),
            Ok(true)
        );
        assert_eq!(
            evaluate_unordered(&has_edge, &graph(vec![]).unwrap(), &FoAssignment::default()),
            Ok(false)
        );
    }

    #[test]
    fn ordered_formula_uses_distinguished_order() {
        let x = Variable(0);
        let y = Variable(1);
        let formula = FoFormula::ForAll {
            variable: x,
            body: Box::new(FoFormula::Exists {
                variable: y,
                body: Box::new(FoFormula::Or(vec![
                    FoFormula::Atom(FoAtom::LessThan(x, y)),
                    FoFormula::Atom(FoAtom::Equal(x, y)),
                ])),
            }),
        };
        let ordered = OrderedFiniteStructure::new(graph(vec![]).unwrap(), vec![2, 0, 1]).unwrap();

        assert_eq!(
            evaluate_ordered(&formula, &ordered, &FoAssignment::default()),
            Ok(true)
        );
        assert_eq!(
            evaluate_unordered(&formula, ordered.structure(), &FoAssignment::default()),
            Err(FoEvaluationError::Formula(
                FoValidationError::OrderNotAvailable
            ))
        );
    }

    #[test]
    fn free_variables_require_valid_assignments() {
        let x = Variable(0);
        let y = Variable(1);
        let edge = FoFormula::Atom(FoAtom::Relation {
            name: "E".into(),
            args: vec![x, y],
        });
        let structure = graph(vec![vec![0, 1]]).unwrap();

        let assignment = FoAssignment::new(vec![(x, 0), (y, 1)]).unwrap();
        assert_eq!(evaluate_unordered(&edge, &structure, &assignment), Ok(true));

        let missing = FoAssignment::new(vec![(x, 0)]).unwrap();
        assert_eq!(
            evaluate_unordered(&edge, &structure, &missing),
            Err(FoEvaluationError::UnboundVariable(y))
        );

        let out_of_domain = FoAssignment::new(vec![(x, 3), (y, 1)]).unwrap();
        assert!(matches!(
            evaluate_unordered(&edge, &structure, &out_of_domain),
            Err(FoEvaluationError::AssignmentOutOfDomain { .. })
        ));
    }

    #[test]
    fn quantifier_shadowing_restores_outer_assignment() {
        let x = Variable(0);
        let formula = FoFormula::And(vec![
            FoFormula::Exists {
                variable: x,
                body: Box::new(FoFormula::Atom(FoAtom::Equal(x, x))),
            },
            FoFormula::Atom(FoAtom::Equal(x, x)),
        ]);
        let assignment = FoAssignment::new(vec![(x, 2)]).unwrap();

        assert_eq!(
            evaluate_unordered(&formula, &graph(vec![]).unwrap(), &assignment),
            Ok(true)
        );
        assert_eq!(assignment.get(x), Some(2));
    }

    #[test]
    fn duplicate_bindings_fail_closed() {
        let x = Variable(0);
        assert_eq!(
            FoAssignment::new(vec![(x, 0), (x, 1)]),
            Err(FoEvaluationError::DuplicateBinding(x))
        );
    }
}
