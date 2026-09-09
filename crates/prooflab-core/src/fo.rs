//! First-order relational syntax for the PL-DC programme.
//!
//! The syntax is intentionally small and explicit. It supports equality,
//! relational atoms, the distinguished order relation, Boolean connectives and
//! first-order quantification. It does not encode ESO or fixed points yet.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{Canonical, CanonicalEncoder, Vocabulary};

/// A syntactic first-order variable identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Variable(pub u64);

impl Canonical for Variable {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.Variable/v1");
        encoder.u64(self.0);
    }
}

/// Atomic formulas over a relational vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoAtom {
    /// Equality between two variables.
    Equal(Variable, Variable),
    /// Application of a relation symbol from the input vocabulary.
    Relation { name: String, args: Vec<Variable> },
    /// Distinguished strict total order atom `left < right`.
    LessThan(Variable, Variable),
}

impl Canonical for FoAtom {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.FoAtom/v1");
        match self {
            Self::Equal(left, right) => {
                encoder.u64(0);
                encoder.value(left);
                encoder.value(right);
            }
            Self::Relation { name, args } => {
                encoder.u64(1);
                encoder.str(name);
                encoder.seq(args);
            }
            Self::LessThan(left, right) => {
                encoder.u64(2);
                encoder.value(left);
                encoder.value(right);
            }
        }
    }
}

/// A first-order formula over finite relational structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoFormula {
    /// Logical truth.
    True,
    /// Logical falsehood.
    False,
    /// Atomic formula.
    Atom(FoAtom),
    /// Negation.
    Not(Box<Self>),
    /// Conjunction. The empty conjunction denotes truth.
    And(Vec<Self>),
    /// Disjunction. The empty disjunction denotes falsehood.
    Or(Vec<Self>),
    /// Existential first-order quantification.
    Exists {
        variable: Variable,
        body: Box<Self>,
    },
    /// Universal first-order quantification.
    ForAll {
        variable: Variable,
        body: Box<Self>,
    },
}

impl FoFormula {
    /// Return the syntactic quantifier rank of the formula.
    #[must_use]
    pub fn quantifier_rank(&self) -> usize {
        match self {
            Self::True | Self::False | Self::Atom(_) => 0,
            Self::Not(body) => body.quantifier_rank(),
            Self::And(parts) | Self::Or(parts) => parts
                .iter()
                .map(Self::quantifier_rank)
                .max()
                .unwrap_or(0),
            Self::Exists { body, .. } | Self::ForAll { body, .. } => {
                body.quantifier_rank().saturating_add(1)
            }
        }
    }

    /// Return the sorted set of syntactic variable identifiers appearing in the formula.
    ///
    /// This is a syntactic variable count, suitable for tracking membership in a
    /// fixed-variable fragment after names have been normalized. It does not claim
    /// that the returned count is the minimum number of variables needed by any
    /// equivalent formula.
    #[must_use]
    pub fn variables(&self) -> Vec<Variable> {
        let mut variables = BTreeSet::new();
        self.collect_variables(&mut variables);
        variables.into_iter().collect()
    }

    /// Return the number of distinct syntactic variable identifiers used.
    #[must_use]
    pub fn variable_count(&self) -> usize {
        self.variables().len()
    }

    /// Return free variables in sorted order.
    #[must_use]
    pub fn free_variables(&self) -> Vec<Variable> {
        let mut free = BTreeSet::new();
        let mut bound = Vec::new();
        self.collect_free_variables(&mut bound, &mut free);
        free.into_iter().collect()
    }

    /// Validate relation atoms against an input vocabulary and an order gate.
    ///
    /// Setting `ordered` to `false` rejects every [`FoAtom::LessThan`] atom. This
    /// keeps unordered experiments from silently acquiring the distinguished
    /// order required by the Immerman-Vardi setting.
    ///
    /// # Errors
    ///
    /// Returns [`FoValidationError`] when a relation is unknown, an atom has the
    /// wrong arity, or an order atom appears while `ordered` is false.
    pub fn validate(
        &self,
        vocabulary: &Vocabulary,
        ordered: bool,
    ) -> Result<(), FoValidationError> {
        match self {
            Self::True | Self::False => Ok(()),
            Self::Atom(atom) => atom.validate(vocabulary, ordered),
            Self::Not(body) | Self::Exists { body, .. } | Self::ForAll { body, .. } => {
                body.validate(vocabulary, ordered)
            }
            Self::And(parts) | Self::Or(parts) => {
                for part in parts {
                    part.validate(vocabulary, ordered)?;
                }
                Ok(())
            }
        }
    }

    fn collect_variables(&self, variables: &mut BTreeSet<Variable>) {
        match self {
            Self::True | Self::False => {}
            Self::Atom(atom) => atom.collect_variables(variables),
            Self::Not(body) => body.collect_variables(variables),
            Self::And(parts) | Self::Or(parts) => {
                for part in parts {
                    part.collect_variables(variables);
                }
            }
            Self::Exists { variable, body } | Self::ForAll { variable, body } => {
                variables.insert(*variable);
                body.collect_variables(variables);
            }
        }
    }

    fn collect_free_variables(
        &self,
        bound: &mut Vec<Variable>,
        free: &mut BTreeSet<Variable>,
    ) {
        match self {
            Self::True | Self::False => {}
            Self::Atom(atom) => atom.collect_free_variables(bound, free),
            Self::Not(body) => body.collect_free_variables(bound, free),
            Self::And(parts) | Self::Or(parts) => {
                for part in parts {
                    part.collect_free_variables(bound, free);
                }
            }
            Self::Exists { variable, body } | Self::ForAll { variable, body } => {
                bound.push(*variable);
                body.collect_free_variables(bound, free);
                let popped = bound.pop();
                debug_assert_eq!(popped, Some(*variable));
            }
        }
    }
}

impl Canonical for FoFormula {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.FoFormula/v1");
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

impl FoAtom {
    fn validate(&self, vocabulary: &Vocabulary, ordered: bool) -> Result<(), FoValidationError> {
        match self {
            Self::Equal(_, _) => Ok(()),
            Self::LessThan(_, _) if ordered => Ok(()),
            Self::LessThan(_, _) => Err(FoValidationError::OrderNotAvailable),
            Self::Relation { name, args } => {
                let symbol = vocabulary
                    .relation(name)
                    .ok_or_else(|| FoValidationError::UnknownRelation(name.clone()))?;
                let expected = usize::try_from(symbol.arity()).ok();
                if expected != Some(args.len()) {
                    return Err(FoValidationError::RelationArity {
                        relation: name.clone(),
                        expected: symbol.arity(),
                        actual: args.len(),
                    });
                }
                Ok(())
            }
        }
    }

    fn collect_variables(&self, variables: &mut BTreeSet<Variable>) {
        match self {
            Self::Equal(left, right) | Self::LessThan(left, right) => {
                variables.insert(*left);
                variables.insert(*right);
            }
            Self::Relation { args, .. } => variables.extend(args.iter().copied()),
        }
    }

    fn collect_free_variables(&self, bound: &[Variable], free: &mut BTreeSet<Variable>) {
        let mut visit = |variable: Variable| {
            if !bound.contains(&variable) {
                free.insert(variable);
            }
        };
        match self {
            Self::Equal(left, right) | Self::LessThan(left, right) => {
                visit(*left);
                visit(*right);
            }
            Self::Relation { args, .. } => {
                for &variable in args {
                    visit(variable);
                }
            }
        }
    }
}

/// Validation failures for first-order formulas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoValidationError {
    /// An atom refers to a relation absent from the input vocabulary.
    UnknownRelation(String),
    /// A relation atom has the wrong arity.
    RelationArity {
        relation: String,
        expected: u64,
        actual: usize,
    },
    /// The distinguished order relation was used in an unordered stage.
    OrderNotAvailable,
}

impl fmt::Display for FoValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRelation(name) => write!(formatter, "unknown relation: {name}"),
            Self::RelationArity {
                relation,
                expected,
                actual,
            } => write!(
                formatter,
                "relation {relation} expects arity {expected}, atom has {actual} arguments"
            ),
            Self::OrderNotAvailable => formatter.write_str(
                "distinguished order atom is not available for an unordered structure",
            ),
        }
    }
}

impl Error for FoValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RelationSymbol;

    fn graph_vocabulary() -> Vocabulary {
        Vocabulary::new(vec![RelationSymbol::new("E", 2).unwrap()]).unwrap()
    }

    #[test]
    fn quantifier_rank_and_variable_count_are_explicit() {
        let x = Variable(0);
        let y = Variable(1);
        let formula = FoFormula::ForAll {
            variable: x,
            body: Box::new(FoFormula::Exists {
                variable: y,
                body: Box::new(FoFormula::And(vec![
                    FoFormula::Atom(FoAtom::Relation {
                        name: "E".into(),
                        args: vec![x, y],
                    }),
                    FoFormula::Atom(FoAtom::LessThan(x, y)),
                ])),
            }),
        };

        assert_eq!(formula.quantifier_rank(), 2);
        assert_eq!(formula.variable_count(), 2);
        assert!(formula.free_variables().is_empty());
        assert_eq!(formula.validate(&graph_vocabulary(), true), Ok(()));
    }

    #[test]
    fn unordered_validation_rejects_order_atoms() {
        let formula = FoFormula::Atom(FoAtom::LessThan(Variable(0), Variable(1)));
        assert_eq!(
            formula.validate(&graph_vocabulary(), false),
            Err(FoValidationError::OrderNotAvailable)
        );
    }

    #[test]
    fn vocabulary_validation_rejects_unknown_and_wrong_arity_atoms() {
        let unknown = FoFormula::Atom(FoAtom::Relation {
            name: "R".into(),
            args: vec![Variable(0)],
        });
        assert_eq!(
            unknown.validate(&graph_vocabulary(), true),
            Err(FoValidationError::UnknownRelation("R".into()))
        );

        let wrong_arity = FoFormula::Atom(FoAtom::Relation {
            name: "E".into(),
            args: vec![Variable(0)],
        });
        assert!(matches!(
            wrong_arity.validate(&graph_vocabulary(), true),
            Err(FoValidationError::RelationArity { .. })
        ));
    }

    #[test]
    fn free_variables_respect_nested_shadowing() {
        let x = Variable(0);
        let y = Variable(1);
        let formula = FoFormula::And(vec![
            FoFormula::Exists {
                variable: x,
                body: Box::new(FoFormula::ForAll {
                    variable: x,
                    body: Box::new(FoFormula::Atom(FoAtom::Equal(x, y))),
                }),
            },
            FoFormula::Atom(FoAtom::Equal(x, x)),
        ]);

        assert_eq!(formula.free_variables(), vec![x, y]);
    }

    #[test]
    fn canonical_formula_encoding_is_constructor_sensitive() {
        let x = Variable(0);
        let atom = FoFormula::Atom(FoAtom::Equal(x, x));
        assert_ne!(
            FoFormula::Not(Box::new(atom.clone())).canonical_bytes(),
            atom.canonical_bytes()
        );
    }
}
