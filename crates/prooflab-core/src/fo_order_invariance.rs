//! Finite order-invariance checks for closed first-order formulas.
//!
//! This module is a falsification oracle for the PL-DC order-invariant track.
//! A caller supplies one unordered structure, one closed FO formula that may use
//! the distinguished order atom, and an explicit family of total orders. The
//! first order is the reference interpretation; any later truth-value change is
//! retained as an exact replay witness.
//!
//! Exhausting a finite family is not a proof of order invariance over all total
//! orders unless the caller independently knows that the supplied family itself
//! exhausts every order of the exact finite carrier.

use core::fmt;

use crate::{
    DescriptiveError, FiniteStructure, FiniteStructureId, FoAssignment, FoEvaluationError,
    FoFormula, OrderedFiniteStructure, OrderedFiniteStructureId, Variable, evaluate_ordered,
};

/// First supplied order whose truth value differs from the reference order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoOrderInvarianceWitness {
    order_index: usize,
    order: Vec<u64>,
    ordered_structure: OrderedFiniteStructureId,
    value: bool,
}

impl FoOrderInvarianceWitness {
    /// Index of this order in the caller-supplied family.
    #[must_use]
    pub const fn order_index(&self) -> usize {
        self.order_index
    }

    /// Exact total order that changed the formula truth value.
    #[must_use]
    pub fn order(&self) -> &[u64] {
        &self.order
    }

    /// Content identity of the ordered expansion used by the witness.
    #[must_use]
    pub const fn ordered_structure(&self) -> OrderedFiniteStructureId {
        self.ordered_structure
    }

    /// Formula truth value under this witness order.
    #[must_use]
    pub const fn value(&self) -> bool {
        self.value
    }
}

/// Deterministic result of checking one explicit finite order family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoOrderInvarianceCheck {
    structure: FiniteStructureId,
    order_count: usize,
    tested_orders: usize,
    reference_order: Vec<u64>,
    reference_ordered_structure: OrderedFiniteStructureId,
    reference_value: bool,
    first_change: Option<FoOrderInvarianceWitness>,
}

impl FoOrderInvarianceCheck {
    /// Content identity of the unordered structure being expanded.
    #[must_use]
    pub const fn structure(&self) -> FiniteStructureId {
        self.structure
    }

    /// Number of orders supplied by the caller.
    #[must_use]
    pub const fn order_count(&self) -> usize {
        self.order_count
    }

    /// Number of orders evaluated before returning the result.
    #[must_use]
    pub const fn tested_orders(&self) -> usize {
        self.tested_orders
    }

    /// First supplied order, used only as a finite reference truth value.
    #[must_use]
    pub fn reference_order(&self) -> &[u64] {
        &self.reference_order
    }

    /// Content identity of the reference ordered expansion.
    #[must_use]
    pub const fn reference_ordered_structure(&self) -> OrderedFiniteStructureId {
        self.reference_ordered_structure
    }

    /// Formula truth value under the first supplied order.
    #[must_use]
    pub const fn reference_value(&self) -> bool {
        self.reference_value
    }

    /// First supplied order whose truth value differs from the reference.
    #[must_use]
    pub const fn first_change(&self) -> Option<&FoOrderInvarianceWitness> {
        self.first_change.as_ref()
    }

    /// Whether all supplied orders yielded the same truth value.
    ///
    /// This is finite-family agreement only; it is not a universal theorem about
    /// every linear order unless the caller separately established family
    /// exhaustiveness for this exact carrier.
    #[must_use]
    pub const fn agrees_on_supplied_family(&self) -> bool {
        self.first_change.is_none()
    }
}

/// Failure while checking finite order invariance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoOrderInvarianceError {
    /// At least one explicit order is required to define the finite reference.
    EmptyOrderFamily,
    /// Order-invariance is defined here only for closed formulas.
    FormulaHasFreeVariables(Vec<Variable>),
    /// One supplied order failed strict-total-order construction.
    OrderedStructure(DescriptiveError),
    /// Exact FO evaluation failed closed.
    Evaluation(FoEvaluationError),
}

impl fmt::Display for FoOrderInvarianceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOrderFamily => {
                formatter.write_str("FO order-invariance check requires at least one order")
            }
            Self::FormulaHasFreeVariables(variables) => write!(
                formatter,
                "FO order-invariance check requires a closed formula; free variables: {variables:?}"
            ),
            Self::OrderedStructure(error) => {
                write!(formatter, "invalid ordered expansion: {error}")
            }
            Self::Evaluation(error) => {
                write!(formatter, "ordered FO evaluation failed: {error}")
            }
        }
    }
}

impl std::error::Error for FoOrderInvarianceError {}

/// Check one closed FO formula over an explicitly supplied finite family of orders.
///
/// The first order defines the finite reference value. Every supplied order is
/// evaluated exactly with [`evaluate_ordered`]. The first truth-value change is
/// retained as a replayable witness; evaluation continues through the whole
/// family so `tested_orders == order_count` on every successful result.
///
/// # Errors
///
/// Fails closed for an empty order family, a formula with free variables, any
/// malformed total order, or an FO evaluation failure.
pub fn check_fo_order_invariance(
    formula: &FoFormula,
    structure: &FiniteStructure,
    orders: &[Vec<u64>],
) -> Result<FoOrderInvarianceCheck, FoOrderInvarianceError> {
    if orders.is_empty() {
        return Err(FoOrderInvarianceError::EmptyOrderFamily);
    }

    let free_variables = formula.free_variables();
    if !free_variables.is_empty() {
        return Err(FoOrderInvarianceError::FormulaHasFreeVariables(
            free_variables,
        ));
    }

    let assignment = FoAssignment::default();
    let reference = OrderedFiniteStructure::new(structure.clone(), orders[0].clone())
        .map_err(FoOrderInvarianceError::OrderedStructure)?;
    let reference_value = evaluate_ordered(formula, &reference, &assignment)
        .map_err(FoOrderInvarianceError::Evaluation)?;
    let reference_ordered_structure = reference.id();

    let mut first_change = None;
    for (order_index, order) in orders.iter().enumerate().skip(1) {
        let ordered = OrderedFiniteStructure::new(structure.clone(), order.clone())
            .map_err(FoOrderInvarianceError::OrderedStructure)?;
        let value = evaluate_ordered(formula, &ordered, &assignment)
            .map_err(FoOrderInvarianceError::Evaluation)?;
        if first_change.is_none() && value != reference_value {
            first_change = Some(FoOrderInvarianceWitness {
                order_index,
                order: order.clone(),
                ordered_structure: ordered.id(),
                value,
            });
        }
    }

    Ok(FoOrderInvarianceCheck {
        structure: structure.id(),
        order_count: orders.len(),
        tested_orders: orders.len(),
        reference_order: orders[0].clone(),
        reference_ordered_structure,
        reference_value,
        first_change,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FoAtom, RelationInterpretation, RelationSymbol, Vocabulary};

    fn marked_structure() -> FiniteStructure {
        let marked = RelationSymbol::new("P", 1).unwrap();
        let vocabulary = Vocabulary::new(vec![marked.clone()]).unwrap();
        let interpretation = RelationInterpretation::new(marked, vec![vec![0]]).unwrap();
        FiniteStructure::new(3, vocabulary, vec![interpretation]).unwrap()
    }

    fn marked_element_is_least() -> FoFormula {
        let x = Variable(0);
        let y = Variable(1);
        FoFormula::Exists {
            variable: x,
            body: Box::new(FoFormula::And(vec![
                FoFormula::Atom(FoAtom::Relation {
                    name: "P".into(),
                    args: vec![x],
                }),
                FoFormula::ForAll {
                    variable: y,
                    body: Box::new(FoFormula::Or(vec![
                        FoFormula::Atom(FoAtom::Equal(x, y)),
                        FoFormula::Atom(FoAtom::LessThan(x, y)),
                    ])),
                },
            ])),
        }
    }

    fn a_least_element_exists() -> FoFormula {
        let x = Variable(0);
        let y = Variable(1);
        FoFormula::Exists {
            variable: x,
            body: Box::new(FoFormula::ForAll {
                variable: y,
                body: Box::new(FoFormula::Or(vec![
                    FoFormula::Atom(FoAtom::Equal(x, y)),
                    FoFormula::Atom(FoAtom::LessThan(x, y)),
                ])),
            }),
        }
    }

    #[test]
    fn order_sensitive_sentence_returns_first_counterexample() {
        let structure = marked_structure();
        let orders = vec![vec![0, 1, 2], vec![0, 2, 1], vec![2, 1, 0]];
        let check =
            check_fo_order_invariance(&marked_element_is_least(), &structure, &orders).unwrap();

        assert!(check.reference_value());
        assert_eq!(check.order_count(), 3);
        assert_eq!(check.tested_orders(), 3);
        assert!(!check.agrees_on_supplied_family());
        let witness = check.first_change().unwrap();
        assert_eq!(witness.order_index(), 2);
        assert_eq!(witness.order(), &[2, 1, 0]);
        assert!(!witness.value());
        assert_ne!(
            witness.ordered_structure(),
            check.reference_ordered_structure()
        );
    }

    #[test]
    fn order_tautology_agrees_on_supplied_family() {
        let structure = marked_structure();
        let orders = vec![vec![0, 1, 2], vec![2, 0, 1], vec![1, 2, 0]];
        let check =
            check_fo_order_invariance(&a_least_element_exists(), &structure, &orders).unwrap();

        assert!(check.reference_value());
        assert!(check.agrees_on_supplied_family());
        assert!(check.first_change().is_none());
        assert_eq!(check.tested_orders(), orders.len());
    }

    #[test]
    fn free_variables_fail_before_order_evaluation() {
        let x = Variable(0);
        let formula = FoFormula::Atom(FoAtom::Equal(x, x));
        assert_eq!(
            check_fo_order_invariance(&formula, &marked_structure(), &[vec![0, 1, 2]]),
            Err(FoOrderInvarianceError::FormulaHasFreeVariables(vec![x]))
        );
    }

    #[test]
    fn empty_and_malformed_order_families_fail_closed() {
        let formula = a_least_element_exists();
        let structure = marked_structure();
        assert_eq!(
            check_fo_order_invariance(&formula, &structure, &[]),
            Err(FoOrderInvarianceError::EmptyOrderFamily)
        );
        assert!(matches!(
            check_fo_order_invariance(&formula, &structure, &[vec![0, 1]]),
            Err(FoOrderInvarianceError::OrderedStructure(
                DescriptiveError::OrderCardinality { .. }
            ))
        ));
    }

    #[test]
    fn check_is_deterministic() {
        let structure = marked_structure();
        let orders = vec![vec![0, 1, 2], vec![2, 1, 0]];
        let first =
            check_fo_order_invariance(&marked_element_is_least(), &structure, &orders).unwrap();
        let second =
            check_fo_order_invariance(&marked_element_is_least(), &structure, &orders).unwrap();
        assert_eq!(first, second);
    }
}
