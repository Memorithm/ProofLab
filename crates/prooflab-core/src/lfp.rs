//! Positive least-fixed-point definition substrate for PL-DC.
//!
//! This module models one non-nested least-fixed-point relation definition
//! `R(x̄) := φ(R, x̄, ȳ)` where recursive occurrences of `R` must be positive.
//! The positivity gate is the syntactic condition used to ensure the induced
//! operator is monotone. Nested FO(LFP) syntax and fixed-point evaluation are
//! deliberately deferred to later slices.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    Canonical, CanonicalEncoder, FoAtom, FoFormula, FoValidationError, Variable, Vocabulary,
};

/// Atomic formulas allowed inside a single positive LFP definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LfpAtom {
    /// An ordinary first-order atom over the input structure.
    FirstOrder(FoAtom),
    /// An occurrence of the recursively defined relation.
    Recursive(Vec<Variable>),
}

impl Canonical for LfpAtom {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.LfpAtom/v1");
        match self {
            Self::FirstOrder(atom) => {
                encoder.u64(0);
                encoder.value(atom);
            }
            Self::Recursive(args) => {
                encoder.u64(1);
                encoder.seq(args);
            }
        }
    }
}

/// First-order body syntax with a distinguished recursive relation atom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LfpBody {
    /// Logical truth.
    True,
    /// Logical falsehood.
    False,
    /// Atomic formula.
    Atom(LfpAtom),
    /// Negation. Recursive atoms below an odd number of negations are rejected.
    Not(Box<Self>),
    /// Conjunction.
    And(Vec<Self>),
    /// Disjunction.
    Or(Vec<Self>),
    /// Existential first-order quantification.
    Exists { variable: Variable, body: Box<Self> },
    /// Universal first-order quantification.
    ForAll { variable: Variable, body: Box<Self> },
}

impl Canonical for LfpBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.LfpBody/v1");
        match self {
            Self::True => encoder.u64(0),
            Self::False => encoder.u64(1),
            Self::Atom(atom) => {
                encoder.u64(2);
                encoder.value(atom);
            }
            Self::Not(body) => {
                encoder.u64(3);
                encoder.value(body.as_ref());
            }
            Self::And(parts) => {
                encoder.u64(4);
                encoder.seq(parts);
            }
            Self::Or(parts) => {
                encoder.u64(5);
                encoder.seq(parts);
            }
            Self::Exists { variable, body } => {
                encoder.u64(6);
                encoder.value(variable);
                encoder.value(body.as_ref());
            }
            Self::ForAll { variable, body } => {
                encoder.u64(7);
                encoder.value(variable);
                encoder.value(body.as_ref());
            }
        }
    }
}

/// One positive, non-nested least-fixed-point relation definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfpDefinition {
    parameters: Vec<Variable>,
    body: LfpBody,
}

impl LfpDefinition {
    /// Construct a least-fixed-point definition.
    ///
    /// `parameters` are the tuple variables of the recursively defined relation.
    /// They must be pairwise distinct. Recursive atoms must have exactly this
    /// arity and occur only positively.
    ///
    /// # Errors
    ///
    /// Returns [`LfpValidationError`] for duplicate tuple variables, recursive
    /// arity mismatches, or negative recursive occurrences.
    pub fn new(parameters: Vec<Variable>, body: LfpBody) -> Result<Self, LfpValidationError> {
        let mut seen = BTreeSet::new();
        for &variable in &parameters {
            if !seen.insert(variable) {
                return Err(LfpValidationError::DuplicateParameter(variable));
            }
        }
        validate_recursive_shape(&body, parameters.len(), true)?;
        Ok(Self { parameters, body })
    }

    /// Return tuple variables of the recursively defined relation.
    #[must_use]
    pub fn parameters(&self) -> &[Variable] {
        &self.parameters
    }

    /// Return the recursive relation arity.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.parameters.len()
    }

    /// Return the defining body.
    #[must_use]
    pub const fn body(&self) -> &LfpBody {
        &self.body
    }

    /// Validate all ordinary FO atoms against an input vocabulary and order mode.
    ///
    /// Recursive shape and positivity were already checked by [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns [`LfpValidationError::FirstOrder`] when an ordinary atom is
    /// invalid for the supplied vocabulary or order mode.
    pub fn validate(
        &self,
        vocabulary: &Vocabulary,
        ordered: bool,
    ) -> Result<(), LfpValidationError> {
        validate_fo_atoms(&self.body, vocabulary, ordered)
    }

    /// Validate an application tuple against this fixed-point relation arity.
    ///
    /// # Errors
    ///
    /// Returns [`LfpValidationError::ApplicationArity`] on mismatch.
    pub fn validate_application(&self, args: &[Variable]) -> Result<(), LfpValidationError> {
        if args.len() != self.arity() {
            return Err(LfpValidationError::ApplicationArity {
                expected: self.arity(),
                actual: args.len(),
            });
        }
        Ok(())
    }
}

impl Canonical for LfpDefinition {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.LfpDefinition/v1");
        encoder.seq(&self.parameters);
        encoder.value(&self.body);
    }
}

fn validate_recursive_shape(
    body: &LfpBody,
    expected_arity: usize,
    positive: bool,
) -> Result<(), LfpValidationError> {
    match body {
        LfpBody::True | LfpBody::False => Ok(()),
        LfpBody::Atom(LfpAtom::FirstOrder(_)) => Ok(()),
        LfpBody::Atom(LfpAtom::Recursive(args)) => {
            if args.len() != expected_arity {
                return Err(LfpValidationError::RecursiveArity {
                    expected: expected_arity,
                    actual: args.len(),
                });
            }
            if !positive {
                return Err(LfpValidationError::NegativeRecursiveOccurrence);
            }
            Ok(())
        }
        LfpBody::Not(inner) => validate_recursive_shape(inner, expected_arity, !positive),
        LfpBody::And(parts) | LfpBody::Or(parts) => {
            for part in parts {
                validate_recursive_shape(part, expected_arity, positive)?;
            }
            Ok(())
        }
        LfpBody::Exists { body, .. } | LfpBody::ForAll { body, .. } => {
            validate_recursive_shape(body, expected_arity, positive)
        }
    }
}

fn validate_fo_atoms(
    body: &LfpBody,
    vocabulary: &Vocabulary,
    ordered: bool,
) -> Result<(), LfpValidationError> {
    match body {
        LfpBody::True | LfpBody::False | LfpBody::Atom(LfpAtom::Recursive(_)) => Ok(()),
        LfpBody::Atom(LfpAtom::FirstOrder(atom)) => FoFormula::Atom(atom.clone())
            .validate(vocabulary, ordered)
            .map_err(LfpValidationError::FirstOrder),
        LfpBody::Not(inner) => validate_fo_atoms(inner, vocabulary, ordered),
        LfpBody::And(parts) | LfpBody::Or(parts) => {
            for part in parts {
                validate_fo_atoms(part, vocabulary, ordered)?;
            }
            Ok(())
        }
        LfpBody::Exists { body, .. } | LfpBody::ForAll { body, .. } => {
            validate_fo_atoms(body, vocabulary, ordered)
        }
    }
}

/// Validation failures for the initial positive LFP substrate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LfpValidationError {
    /// The recursive tuple binder repeats one first-order variable.
    DuplicateParameter(Variable),
    /// A recursive relation atom has the wrong tuple arity.
    RecursiveArity { expected: usize, actual: usize },
    /// A recursive relation occurrence is under an odd number of negations.
    NegativeRecursiveOccurrence,
    /// An ordinary first-order atom is invalid for the input/order mode.
    FirstOrder(FoValidationError),
    /// A fixed-point application uses the wrong tuple arity.
    ApplicationArity { expected: usize, actual: usize },
}

impl fmt::Display for LfpValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateParameter(variable) => {
                write!(formatter, "duplicate LFP tuple variable {}", variable.0)
            }
            Self::RecursiveArity { expected, actual } => write!(
                formatter,
                "recursive LFP atom expects arity {expected}, found {actual} arguments"
            ),
            Self::NegativeRecursiveOccurrence => {
                formatter.write_str("recursive LFP relation occurs negatively in its defining body")
            }
            Self::FirstOrder(error) => write!(formatter, "invalid FO atom in LFP body: {error}"),
            Self::ApplicationArity { expected, actual } => write!(
                formatter,
                "LFP application expects arity {expected}, found {actual} arguments"
            ),
        }
    }
}

impl Error for LfpValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RelationSymbol;

    fn graph_vocabulary() -> Vocabulary {
        Vocabulary::new(vec![RelationSymbol::new("E", 2).unwrap()]).unwrap()
    }

    #[test]
    fn reachability_shape_is_positive_and_valid() {
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
        let definition = LfpDefinition::new(vec![x, y], body).unwrap();

        assert_eq!(definition.arity(), 2);
        assert_eq!(definition.validate(&graph_vocabulary(), false), Ok(()));
        assert_eq!(definition.validate_application(&[x, y]), Ok(()));
    }

    #[test]
    fn negative_recursive_occurrence_is_rejected() {
        let x = Variable(0);
        let body = LfpBody::Not(Box::new(LfpBody::Atom(LfpAtom::Recursive(vec![x]))));

        assert_eq!(
            LfpDefinition::new(vec![x], body),
            Err(LfpValidationError::NegativeRecursiveOccurrence)
        );
    }

    #[test]
    fn double_negation_restores_positive_polarity() {
        let x = Variable(0);
        let body = LfpBody::Not(Box::new(LfpBody::Not(Box::new(LfpBody::Atom(
            LfpAtom::Recursive(vec![x]),
        )))));

        assert!(LfpDefinition::new(vec![x], body).is_ok());
    }

    #[test]
    fn recursive_and_application_arities_fail_closed() {
        let x = Variable(0);
        let y = Variable(1);
        let wrong_recursive = LfpBody::Atom(LfpAtom::Recursive(vec![x]));
        assert_eq!(
            LfpDefinition::new(vec![x, y], wrong_recursive),
            Err(LfpValidationError::RecursiveArity {
                expected: 2,
                actual: 1,
            })
        );

        let definition = LfpDefinition::new(vec![x, y], LfpBody::True).unwrap();
        assert_eq!(
            definition.validate_application(&[x]),
            Err(LfpValidationError::ApplicationArity {
                expected: 2,
                actual: 1,
            })
        );
    }

    #[test]
    fn duplicate_tuple_variables_are_rejected() {
        let x = Variable(0);
        assert_eq!(
            LfpDefinition::new(vec![x, x], LfpBody::True),
            Err(LfpValidationError::DuplicateParameter(x))
        );
    }

    #[test]
    fn order_gate_is_preserved_in_lfp_body() {
        let x = Variable(0);
        let y = Variable(1);
        let definition = LfpDefinition::new(
            vec![x, y],
            LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::LessThan(x, y))),
        )
        .unwrap();

        assert_eq!(
            definition.validate(&graph_vocabulary(), false),
            Err(LfpValidationError::FirstOrder(
                FoValidationError::OrderNotAvailable
            ))
        );
        assert_eq!(definition.validate(&graph_vocabulary(), true), Ok(()));
    }
}
