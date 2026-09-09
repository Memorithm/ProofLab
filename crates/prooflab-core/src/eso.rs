//! Existential second-order relational syntax for the PL-DC programme.
//!
//! An [`EsoSentence`] is represented in the standard relational prenex shape
//! `∃R₁ ... ∃Rₖ. φ`, where the witnesses are fresh relation symbols and `φ` is
//! a closed first-order formula over the input vocabulary extended by those
//! witnesses. This module defines syntax and validation only; it does not infer
//! a witness or prove a complexity-theoretic characterization.

use std::error::Error;
use std::fmt;

use crate::{
    Canonical, CanonicalEncoder, DescriptiveError, FoFormula, FoValidationError, RelationSymbol,
    Variable, Vocabulary,
};

/// A closed existential second-order sentence with relational witnesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsoSentence {
    witnesses: Vocabulary,
    body: FoFormula,
}

impl EsoSentence {
    /// Construct `∃R₁ ... ∃Rₖ. body` from fresh relational witness symbols.
    ///
    /// The first-order body must already be syntactically closed. Freshness of
    /// witness names relative to a concrete input vocabulary is checked by
    /// [`Self::validate`].
    ///
    /// # Errors
    ///
    /// Returns [`EsoValidationError::WitnessVocabulary`] if witness symbols are
    /// malformed or duplicated, and [`EsoValidationError::FreeFirstOrderVariables`]
    /// if `body` contains free first-order variables.
    pub fn new(
        witnesses: Vec<RelationSymbol>,
        body: FoFormula,
    ) -> Result<Self, EsoValidationError> {
        let witnesses =
            Vocabulary::new(witnesses).map_err(EsoValidationError::WitnessVocabulary)?;
        let free = body.free_variables();
        if !free.is_empty() {
            return Err(EsoValidationError::FreeFirstOrderVariables(free));
        }
        Ok(Self { witnesses, body })
    }

    /// Return existentially quantified relation symbols in canonical order.
    #[must_use]
    pub const fn witnesses(&self) -> &Vocabulary {
        &self.witnesses
    }

    /// Return the closed first-order matrix.
    #[must_use]
    pub const fn body(&self) -> &FoFormula {
        &self.body
    }

    /// Return the number of existential second-order relation variables.
    #[must_use]
    pub fn witness_count(&self) -> usize {
        self.witnesses.relations().len()
    }

    /// Return the maximum witness arity, or `None` for a witness-free sentence.
    #[must_use]
    pub fn max_witness_arity(&self) -> Option<u64> {
        self.witnesses
            .relations()
            .iter()
            .map(RelationSymbol::arity)
            .max()
    }

    /// Validate this sentence against a concrete input vocabulary and order mode.
    ///
    /// Witness relation names must be fresh relative to the input vocabulary.
    /// The FO matrix is then validated against the disjoint union of input and
    /// witness relations. Setting `ordered` to `false` preserves the same
    /// fail-closed order gate as ordinary FO validation.
    ///
    /// # Errors
    ///
    /// Returns [`EsoValidationError`] for witness/input name collisions,
    /// malformed combined vocabulary, or an invalid first-order body.
    pub fn validate(&self, input: &Vocabulary, ordered: bool) -> Result<(), EsoValidationError> {
        for witness in self.witnesses.relations() {
            if input.relation(witness.name()).is_some() {
                return Err(EsoValidationError::WitnessShadowsInput(
                    witness.name().to_owned(),
                ));
            }
        }

        let mut extended = input.relations().to_vec();
        extended.extend(self.witnesses.relations().iter().cloned());
        let extended = Vocabulary::new(extended).map_err(EsoValidationError::ExtendedVocabulary)?;
        self.body
            .validate(&extended, ordered)
            .map_err(EsoValidationError::Body)
    }
}

impl Canonical for EsoSentence {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.EsoSentence/v1");
        encoder.value(&self.witnesses);
        encoder.value(&self.body);
    }
}

/// Validation failures for relational ESO sentences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EsoValidationError {
    /// The existential witness vocabulary itself is malformed.
    WitnessVocabulary(DescriptiveError),
    /// An ESO sentence must not leave first-order variables free.
    FreeFirstOrderVariables(Vec<Variable>),
    /// A quantified witness relation reuses an input relation name.
    WitnessShadowsInput(String),
    /// Defensive failure while building the disjoint extended vocabulary.
    ExtendedVocabulary(DescriptiveError),
    /// The first-order matrix is invalid over the extended vocabulary/order mode.
    Body(FoValidationError),
}

impl fmt::Display for EsoValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WitnessVocabulary(error) => {
                write!(formatter, "invalid ESO witness vocabulary: {error}")
            }
            Self::FreeFirstOrderVariables(variables) => write!(
                formatter,
                "ESO sentence body contains free first-order variables: {variables:?}"
            ),
            Self::WitnessShadowsInput(name) => {
                write!(
                    formatter,
                    "ESO witness relation shadows input relation {name}"
                )
            }
            Self::ExtendedVocabulary(error) => {
                write!(formatter, "invalid ESO extended vocabulary: {error}")
            }
            Self::Body(error) => write!(formatter, "invalid ESO first-order body: {error}"),
        }
    }
}

impl Error for EsoValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FoAtom, Variable};

    fn graph_vocabulary() -> Vocabulary {
        Vocabulary::new(vec![RelationSymbol::new("E", 2).unwrap()]).unwrap()
    }

    #[test]
    fn witness_relation_extends_input_vocabulary() {
        let x = Variable(0);
        let y = Variable(1);
        let body = FoFormula::ForAll {
            variable: x,
            body: Box::new(FoFormula::ForAll {
                variable: y,
                body: Box::new(FoFormula::Or(vec![
                    FoFormula::Not(Box::new(FoFormula::Atom(FoAtom::Relation {
                        name: "H".into(),
                        args: vec![x, y],
                    }))),
                    FoFormula::Atom(FoAtom::Relation {
                        name: "E".into(),
                        args: vec![x, y],
                    }),
                ])),
            }),
        };
        let sentence = EsoSentence::new(vec![RelationSymbol::new("H", 2).unwrap()], body).unwrap();

        assert_eq!(sentence.witness_count(), 1);
        assert_eq!(sentence.max_witness_arity(), Some(2));
        assert_eq!(sentence.validate(&graph_vocabulary(), false), Ok(()));
    }

    #[test]
    fn witness_names_must_be_fresh_relative_to_input() {
        let sentence =
            EsoSentence::new(vec![RelationSymbol::new("E", 2).unwrap()], FoFormula::True).unwrap();

        assert_eq!(
            sentence.validate(&graph_vocabulary(), false),
            Err(EsoValidationError::WitnessShadowsInput("E".into()))
        );
    }

    #[test]
    fn eso_sentence_rejects_free_first_order_variables() {
        let x = Variable(0);
        let body = FoFormula::Atom(FoAtom::Equal(x, x));

        assert_eq!(
            EsoSentence::new(vec![], body),
            Err(EsoValidationError::FreeFirstOrderVariables(vec![x]))
        );
    }

    #[test]
    fn order_gate_is_preserved_inside_eso() {
        let x = Variable(0);
        let y = Variable(1);
        let body = FoFormula::ForAll {
            variable: x,
            body: Box::new(FoFormula::ForAll {
                variable: y,
                body: Box::new(FoFormula::Atom(FoAtom::LessThan(x, y))),
            }),
        };
        let sentence = EsoSentence::new(vec![], body).unwrap();

        assert_eq!(
            sentence.validate(&graph_vocabulary(), false),
            Err(EsoValidationError::Body(
                FoValidationError::OrderNotAvailable
            ))
        );
        assert_eq!(sentence.validate(&graph_vocabulary(), true), Ok(()));
    }

    #[test]
    fn canonical_encoding_depends_on_witness_signature() {
        let binary =
            EsoSentence::new(vec![RelationSymbol::new("H", 2).unwrap()], FoFormula::True).unwrap();
        let unary =
            EsoSentence::new(vec![RelationSymbol::new("H", 1).unwrap()], FoFormula::True).unwrap();

        assert_ne!(binary.canonical_bytes(), unary.canonical_bytes());
    }
}
