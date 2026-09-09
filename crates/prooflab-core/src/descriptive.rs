//! Finite relational structures for the PL-DC descriptive-complexity programme.
//!
//! This module deliberately distinguishes an unordered finite structure from a
//! finite structure equipped with an explicit total order. The distinction is a
//! scientific invariant for the ordered `FO(LFP)` versus `ESO` research line:
//! results obtained on unordered structures must not be silently promoted to
//! the ordered setting.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{Canonical, CanonicalEncoder, sha256_canonical};

const FINITE_STRUCTURE_DOMAIN: &[u8] = b"prooflab:finite-structure:v1\0";
const ORDERED_STRUCTURE_DOMAIN: &[u8] = b"prooflab:ordered-finite-structure:v1\0";

/// A relation symbol in a finite relational vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationSymbol {
    name: String,
    arity: u64,
}

impl RelationSymbol {
    /// Construct a relation symbol.
    ///
    /// Zero-arity relation symbols are permitted; an empty name is not.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptiveError::EmptyRelationName`] when `name` is empty.
    pub fn new(name: impl Into<String>, arity: u64) -> Result<Self, DescriptiveError> {
        let name = name.into();
        if name.is_empty() {
            return Err(DescriptiveError::EmptyRelationName);
        }
        Ok(Self { name, arity })
    }

    /// Return the relation name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the relation arity.
    #[must_use]
    pub const fn arity(&self) -> u64 {
        self.arity
    }
}

impl Canonical for RelationSymbol {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.RelationSymbol/v1");
        encoder.str(&self.name);
        encoder.u64(self.arity);
    }
}

/// A deterministic finite relational vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    relations: Vec<RelationSymbol>,
}

impl Vocabulary {
    /// Construct a vocabulary, canonicalizing relation order by symbol name.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptiveError::DuplicateRelationName`] if two relation
    /// symbols have the same name, even when their arities differ.
    pub fn new(mut relations: Vec<RelationSymbol>) -> Result<Self, DescriptiveError> {
        relations.sort_by(|left, right| left.name.cmp(&right.name));
        for pair in relations.windows(2) {
            if pair[0].name == pair[1].name {
                return Err(DescriptiveError::DuplicateRelationName(
                    pair[0].name.clone(),
                ));
            }
        }
        Ok(Self { relations })
    }

    /// Return relation symbols in canonical order.
    #[must_use]
    pub fn relations(&self) -> &[RelationSymbol] {
        &self.relations
    }

    /// Look up a relation symbol by name.
    #[must_use]
    pub fn relation(&self, name: &str) -> Option<&RelationSymbol> {
        self.relations
            .binary_search_by(|symbol| symbol.name.as_str().cmp(name))
            .ok()
            .map(|index| &self.relations[index])
    }
}

impl Canonical for Vocabulary {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.Vocabulary/v1");
        encoder.seq(&self.relations);
    }
}

/// The finite interpretation of one relation symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationInterpretation {
    symbol: RelationSymbol,
    tuples: Vec<Vec<u64>>,
}

impl RelationInterpretation {
    /// Construct an interpretation and canonicalize tuple order.
    ///
    /// Domain bounds are checked when the interpretation is attached to a
    /// [`FiniteStructure`].
    ///
    /// # Errors
    ///
    /// Returns [`DescriptiveError::ArityNotAddressable`] if the declared arity
    /// cannot be represented by the current platform, [`DescriptiveError::TupleArity`]
    /// for a tuple with the wrong arity, or [`DescriptiveError::DuplicateTuple`]
    /// for a repeated tuple.
    pub fn new(
        symbol: RelationSymbol,
        mut tuples: Vec<Vec<u64>>,
    ) -> Result<Self, DescriptiveError> {
        let expected =
            usize::try_from(symbol.arity).map_err(|_| DescriptiveError::ArityNotAddressable {
                relation: symbol.name.clone(),
                arity: symbol.arity,
            })?;

        for tuple in &tuples {
            if tuple.len() != expected {
                return Err(DescriptiveError::TupleArity {
                    relation: symbol.name.clone(),
                    expected: symbol.arity,
                    actual: tuple.len(),
                });
            }
        }

        tuples.sort();
        for pair in tuples.windows(2) {
            if pair[0] == pair[1] {
                return Err(DescriptiveError::DuplicateTuple {
                    relation: symbol.name.clone(),
                    tuple: pair[0].clone(),
                });
            }
        }

        Ok(Self { symbol, tuples })
    }

    /// Return the interpreted relation symbol.
    #[must_use]
    pub const fn symbol(&self) -> &RelationSymbol {
        &self.symbol
    }

    /// Return tuples in canonical lexicographic order.
    #[must_use]
    pub fn tuples(&self) -> &[Vec<u64>] {
        &self.tuples
    }

    /// Test membership of an exact tuple.
    #[must_use]
    pub fn contains(&self, tuple: &[u64]) -> bool {
        self.tuples
            .binary_search_by(|candidate| candidate.as_slice().cmp(tuple))
            .is_ok()
    }
}

impl Canonical for RelationInterpretation {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.RelationInterpretation/v1");
        encoder.value(&self.symbol);
        encoder.seq(&self.tuples);
    }
}

/// A finite relational structure without a distinguished order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteStructure {
    domain_size: u64,
    vocabulary: Vocabulary,
    relations: Vec<RelationInterpretation>,
}

impl FiniteStructure {
    /// Construct a non-empty finite relational structure.
    ///
    /// Relation interpretations are canonicalized into vocabulary order.
    /// Exactly one interpretation must be supplied for every relation symbol.
    ///
    /// # Errors
    ///
    /// Returns an error when the domain is empty, an interpretation is missing,
    /// duplicated or unexpected, or a tuple references an element outside the
    /// finite domain `0 .. domain_size`.
    pub fn new(
        domain_size: u64,
        vocabulary: Vocabulary,
        mut relations: Vec<RelationInterpretation>,
    ) -> Result<Self, DescriptiveError> {
        if domain_size == 0 {
            return Err(DescriptiveError::EmptyDomain);
        }

        relations.sort_by(|left, right| left.symbol.name.cmp(&right.symbol.name));
        for pair in relations.windows(2) {
            if pair[0].symbol.name == pair[1].symbol.name {
                return Err(DescriptiveError::DuplicateInterpretation(
                    pair[0].symbol.name.clone(),
                ));
            }
        }

        for interpretation in &relations {
            let Some(expected_symbol) = vocabulary.relation(&interpretation.symbol.name) else {
                return Err(DescriptiveError::UnexpectedInterpretation(
                    interpretation.symbol.name.clone(),
                ));
            };
            if expected_symbol != &interpretation.symbol {
                return Err(DescriptiveError::InterpretationSignatureMismatch {
                    relation: interpretation.symbol.name.clone(),
                    expected_arity: expected_symbol.arity,
                    actual_arity: interpretation.symbol.arity,
                });
            }
            for tuple in &interpretation.tuples {
                if let Some(&element) = tuple.iter().find(|&&element| element >= domain_size) {
                    return Err(DescriptiveError::ElementOutOfDomain {
                        relation: interpretation.symbol.name.clone(),
                        element,
                        domain_size,
                    });
                }
            }
        }

        for symbol in vocabulary.relations() {
            if relations
                .binary_search_by(|interpretation| interpretation.symbol.name.cmp(&symbol.name))
                .is_err()
            {
                return Err(DescriptiveError::MissingInterpretation(symbol.name.clone()));
            }
        }

        Ok(Self {
            domain_size,
            vocabulary,
            relations,
        })
    }

    /// Return the number of domain elements.
    #[must_use]
    pub const fn domain_size(&self) -> u64 {
        self.domain_size
    }

    /// Return the relational vocabulary.
    #[must_use]
    pub const fn vocabulary(&self) -> &Vocabulary {
        &self.vocabulary
    }

    /// Return interpretations in canonical vocabulary order.
    #[must_use]
    pub fn relations(&self) -> &[RelationInterpretation] {
        &self.relations
    }

    /// Look up a relation interpretation by symbol name.
    #[must_use]
    pub fn relation(&self, name: &str) -> Option<&RelationInterpretation> {
        self.relations
            .binary_search_by(|interpretation| interpretation.symbol.name.as_str().cmp(name))
            .ok()
            .map(|index| &self.relations[index])
    }

    /// Return the content identity of this exact finite structure.
    #[must_use]
    pub fn id(&self) -> FiniteStructureId {
        FiniteStructureId(sha256_canonical(FINITE_STRUCTURE_DOMAIN, self))
    }
}

impl Canonical for FiniteStructure {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.FiniteStructure/v1");
        encoder.u64(self.domain_size);
        encoder.value(&self.vocabulary);
        encoder.seq(&self.relations);
    }
}

/// Content-addressed identity of a [`FiniteStructure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FiniteStructureId(pub [u8; 32]);

/// A finite structure equipped with an explicit strict total order.
///
/// `order` is stored as the sequence of domain elements from least to greatest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedFiniteStructure {
    structure: FiniteStructure,
    order: Vec<u64>,
}

impl OrderedFiniteStructure {
    /// Attach a strict total order to a finite structure.
    ///
    /// # Errors
    ///
    /// Returns an error unless `order` is a permutation of the complete domain.
    pub fn new(structure: FiniteStructure, order: Vec<u64>) -> Result<Self, DescriptiveError> {
        let expected = usize::try_from(structure.domain_size)
            .map_err(|_| DescriptiveError::DomainNotAddressable(structure.domain_size))?;
        if order.len() != expected {
            return Err(DescriptiveError::OrderCardinality {
                expected: structure.domain_size,
                actual: order.len(),
            });
        }

        let mut seen = BTreeSet::new();
        for &element in &order {
            if element >= structure.domain_size {
                return Err(DescriptiveError::OrderElementOutOfDomain {
                    element,
                    domain_size: structure.domain_size,
                });
            }
            if !seen.insert(element) {
                return Err(DescriptiveError::DuplicateOrderElement(element));
            }
        }

        Ok(Self { structure, order })
    }

    /// Return the underlying unordered relational structure.
    #[must_use]
    pub const fn structure(&self) -> &FiniteStructure {
        &self.structure
    }

    /// Return elements from least to greatest.
    #[must_use]
    pub fn order(&self) -> &[u64] {
        &self.order
    }

    /// Evaluate the distinguished strict order relation.
    #[must_use]
    pub fn less_than(&self, left: u64, right: u64) -> bool {
        let left_position = self.order.iter().position(|&value| value == left);
        let right_position = self.order.iter().position(|&value| value == right);
        matches!((left_position, right_position), (Some(left), Some(right)) if left < right)
    }

    /// Return the content identity including the distinguished order.
    #[must_use]
    pub fn id(&self) -> OrderedFiniteStructureId {
        OrderedFiniteStructureId(sha256_canonical(ORDERED_STRUCTURE_DOMAIN, self))
    }
}

impl Canonical for OrderedFiniteStructure {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.OrderedFiniteStructure/v1");
        encoder.value(&self.structure);
        encoder.seq(&self.order);
    }
}

/// Content-addressed identity of an [`OrderedFiniteStructure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderedFiniteStructureId(pub [u8; 32]);

/// Validation errors for finite descriptive-complexity structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptiveError {
    /// Relation names must be non-empty.
    EmptyRelationName,
    /// A vocabulary contains the same relation name twice.
    DuplicateRelationName(String),
    /// The declared relation arity does not fit the platform address space.
    ArityNotAddressable { relation: String, arity: u64 },
    /// A relation tuple has the wrong number of components.
    TupleArity {
        relation: String,
        expected: u64,
        actual: usize,
    },
    /// The same tuple appears more than once in an interpretation.
    DuplicateTuple { relation: String, tuple: Vec<u64> },
    /// First-order structures in this substrate are required to be non-empty.
    EmptyDomain,
    /// A domain is too large to materialize as an explicit order on this platform.
    DomainNotAddressable(u64),
    /// More than one interpretation was supplied for one relation name.
    DuplicateInterpretation(String),
    /// An interpretation does not belong to the declared vocabulary.
    UnexpectedInterpretation(String),
    /// A vocabulary relation has no interpretation.
    MissingInterpretation(String),
    /// An interpretation reuses a name with an arity different from the vocabulary.
    InterpretationSignatureMismatch {
        relation: String,
        expected_arity: u64,
        actual_arity: u64,
    },
    /// A tuple contains an element outside the finite domain.
    ElementOutOfDomain {
        relation: String,
        element: u64,
        domain_size: u64,
    },
    /// The explicit order does not contain exactly one entry per domain element.
    OrderCardinality { expected: u64, actual: usize },
    /// The explicit order references an element outside the finite domain.
    OrderElementOutOfDomain { element: u64, domain_size: u64 },
    /// The explicit order repeats a domain element.
    DuplicateOrderElement(u64),
}

impl fmt::Display for DescriptiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRelationName => formatter.write_str("relation name must not be empty"),
            Self::DuplicateRelationName(name) => {
                write!(formatter, "duplicate relation name: {name}")
            }
            Self::ArityNotAddressable { relation, arity } => write!(
                formatter,
                "relation {relation} has arity {arity}, which is not addressable on this platform"
            ),
            Self::TupleArity {
                relation,
                expected,
                actual,
            } => write!(
                formatter,
                "relation {relation} expects arity {expected}, tuple has {actual} components"
            ),
            Self::DuplicateTuple { relation, tuple } => {
                write!(
                    formatter,
                    "relation {relation} contains duplicate tuple {tuple:?}"
                )
            }
            Self::EmptyDomain => formatter.write_str("finite structure domain must not be empty"),
            Self::DomainNotAddressable(size) => write!(
                formatter,
                "domain size {size} cannot be materialized on this platform"
            ),
            Self::DuplicateInterpretation(name) => {
                write!(formatter, "duplicate interpretation for relation {name}")
            }
            Self::UnexpectedInterpretation(name) => {
                write!(formatter, "unexpected interpretation for relation {name}")
            }
            Self::MissingInterpretation(name) => {
                write!(formatter, "missing interpretation for relation {name}")
            }
            Self::InterpretationSignatureMismatch {
                relation,
                expected_arity,
                actual_arity,
            } => write!(
                formatter,
                "relation {relation} has vocabulary arity {expected_arity} but interpretation arity {actual_arity}"
            ),
            Self::ElementOutOfDomain {
                relation,
                element,
                domain_size,
            } => write!(
                formatter,
                "relation {relation} references element {element} outside domain size {domain_size}"
            ),
            Self::OrderCardinality { expected, actual } => write!(
                formatter,
                "order must contain {expected} elements, found {actual}"
            ),
            Self::OrderElementOutOfDomain {
                element,
                domain_size,
            } => write!(
                formatter,
                "order references element {element} outside domain size {domain_size}"
            ),
            Self::DuplicateOrderElement(element) => {
                write!(formatter, "order repeats domain element {element}")
            }
        }
    }
}

impl Error for DescriptiveError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_vocabulary() -> Vocabulary {
        Vocabulary::new(vec![RelationSymbol::new("E", 2).unwrap()]).unwrap()
    }

    fn graph(edge_tuples: Vec<Vec<u64>>) -> FiniteStructure {
        let edge = RelationSymbol::new("E", 2).unwrap();
        FiniteStructure::new(
            3,
            graph_vocabulary(),
            vec![RelationInterpretation::new(edge, edge_tuples).unwrap()],
        )
        .unwrap()
    }

    #[test]
    fn vocabulary_and_tuple_order_are_canonical() {
        let alpha = RelationSymbol::new("A", 1).unwrap();
        let edge = RelationSymbol::new("E", 2).unwrap();
        let vocabulary = Vocabulary::new(vec![edge.clone(), alpha.clone()]).unwrap();
        assert_eq!(vocabulary.relations(), &[alpha, edge]);

        let first = graph(vec![vec![2, 0], vec![0, 1]]);
        let second = graph(vec![vec![0, 1], vec![2, 0]]);
        assert_eq!(first, second);
        assert_eq!(first.id(), second.id());
    }

    #[test]
    fn structure_validation_fails_closed() {
        assert_eq!(
            FiniteStructure::new(0, Vocabulary::new(vec![]).unwrap(), vec![]),
            Err(DescriptiveError::EmptyDomain)
        );

        let edge = RelationSymbol::new("E", 2).unwrap();
        let wrong_arity = RelationInterpretation::new(edge.clone(), vec![vec![0]]);
        assert!(matches!(
            wrong_arity,
            Err(DescriptiveError::TupleArity { .. })
        ));

        let out_of_domain = FiniteStructure::new(
            3,
            graph_vocabulary(),
            vec![RelationInterpretation::new(edge, vec![vec![0, 3]]).unwrap()],
        );
        assert!(matches!(
            out_of_domain,
            Err(DescriptiveError::ElementOutOfDomain { .. })
        ));
    }

    #[test]
    fn ordered_and_unordered_structures_are_distinct_objects() {
        let structure = graph(vec![vec![0, 1], vec![1, 2]]);
        let ascending = OrderedFiniteStructure::new(structure.clone(), vec![0, 1, 2]).unwrap();
        let descending = OrderedFiniteStructure::new(structure, vec![2, 1, 0]).unwrap();

        assert!(ascending.less_than(0, 2));
        assert!(!ascending.less_than(2, 0));
        assert!(descending.less_than(2, 0));
        assert_ne!(ascending.id(), descending.id());
    }

    #[test]
    fn total_order_must_be_a_domain_permutation() {
        let structure = graph(vec![]);
        assert_eq!(
            OrderedFiniteStructure::new(structure.clone(), vec![0, 1]),
            Err(DescriptiveError::OrderCardinality {
                expected: 3,
                actual: 2,
            })
        );
        assert_eq!(
            OrderedFiniteStructure::new(structure.clone(), vec![0, 1, 3]),
            Err(DescriptiveError::OrderElementOutOfDomain {
                element: 3,
                domain_size: 3,
            })
        );
        assert_eq!(
            OrderedFiniteStructure::new(structure, vec![0, 1, 1]),
            Err(DescriptiveError::DuplicateOrderElement(1))
        );
    }

    #[test]
    fn relation_membership_is_exact() {
        let structure = graph(vec![vec![0, 1], vec![2, 0]]);
        let edge = structure.relation("E").unwrap();
        assert!(edge.contains(&[0, 1]));
        assert!(!edge.contains(&[1, 0]));
    }
}
