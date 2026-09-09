//! Exact coordinate-wise `k`-dimensional Weisfeiler–Leman refinement.
//!
//! This module implements the coordinate-wise/oblivious `k`-WL convention for
//! `k >= 2`: each tuple color is refined using the current color together with
//! one multiset of replacement colors per tuple coordinate. It deliberately
//! does not call this construction `1-WL`, and it deliberately does not
//! implement folklore `k`-FWL, whose aggregation couples all coordinate
//! replacements for the same replacement element.
//!
//! Two structures are refined jointly so numeric color identifiers are directly
//! comparable across the pair. The initial coloring is the complete atomic type
//! of each tuple: equality, every input relation on every tuple of coordinates,
//! and the distinguished order when ordered mode is requested.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{FiniteStructure, OrderedFiniteStructure};

/// Stable result of one exact joint coordinate-wise `k`-WL refinement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObliviousWlComparison {
    dimension: usize,
    refinement_applications: usize,
    distinguished: bool,
    stable_color_classes: usize,
    left_colors: Vec<u64>,
    right_colors: Vec<u64>,
}

impl ObliviousWlComparison {
    /// Tuple dimension used by the comparison.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Number of refinement applications, including the final stable pass.
    #[must_use]
    pub const fn refinement_applications(&self) -> usize {
        self.refinement_applications
    }

    /// Whether the stable joint color histograms distinguish the structures.
    #[must_use]
    pub const fn distinguished(&self) -> bool {
        self.distinguished
    }

    /// Number of stable color classes across both structures jointly.
    #[must_use]
    pub const fn stable_color_classes(&self) -> usize {
        self.stable_color_classes
    }

    /// Stable colors of left `k`-tuples in lexicographic tuple order.
    #[must_use]
    pub fn left_colors(&self) -> &[u64] {
        &self.left_colors
    }

    /// Stable colors of right `k`-tuples in lexicographic tuple order.
    #[must_use]
    pub fn right_colors(&self) -> &[u64] {
        &self.right_colors
    }
}

/// Run exact coordinate-wise `k`-WL jointly on two unordered finite structures.
///
/// # Errors
///
/// Returns [`ObliviousWlError`] for incompatible vocabularies, dimensions below
/// two, unaddressable explicit tuple spaces/buffers, or violated internal
/// indexing invariants.
pub fn compare_oblivious_wl_unordered(
    left: &FiniteStructure,
    right: &FiniteStructure,
    dimension: usize,
) -> Result<ObliviousWlComparison, ObliviousWlError> {
    compare(left, right, None, None, dimension)
}

/// Run exact coordinate-wise `k`-WL jointly with distinguished orders.
///
/// # Errors
///
/// Returns [`ObliviousWlError`] for incompatible vocabularies, dimensions below
/// two, unaddressable explicit tuple spaces/buffers, or violated internal
/// indexing invariants.
pub fn compare_oblivious_wl_ordered(
    left: &OrderedFiniteStructure,
    right: &OrderedFiniteStructure,
    dimension: usize,
) -> Result<ObliviousWlComparison, ObliviousWlError> {
    compare(
        left.structure(),
        right.structure(),
        Some(left),
        Some(right),
        dimension,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AtomicSignature(Vec<bool>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RefinementSignature {
    current: u64,
    coordinate_multisets: Vec<Vec<u64>>,
}

fn compare(
    left: &FiniteStructure,
    right: &FiniteStructure,
    left_order: Option<&OrderedFiniteStructure>,
    right_order: Option<&OrderedFiniteStructure>,
    dimension: usize,
) -> Result<ObliviousWlComparison, ObliviousWlError> {
    if dimension < 2 {
        return Err(ObliviousWlError::DimensionTooSmall(dimension));
    }
    if left.vocabulary() != right.vocabulary() {
        return Err(ObliviousWlError::VocabularyMismatch);
    }
    if left_order.is_some() != right_order.is_some() {
        return Err(ObliviousWlError::OrderModeMismatch);
    }

    ensure_tuple_buffer(dimension)?;
    let left_tuple_count = checked_tuple_count(left.domain_size(), dimension)?;
    let right_tuple_count = checked_tuple_count(right.domain_size(), dimension)?;

    let left_tuples = enumerate_tuples(left.domain_size(), dimension, left_tuple_count)?;
    let right_tuples = enumerate_tuples(right.domain_size(), dimension, right_tuple_count)?;

    let left_atomic = atomic_signatures(left, left_order, &left_tuples, dimension)?;
    let right_atomic = atomic_signatures(right, right_order, &right_tuples, dimension)?;
    let (mut left_colors, mut right_colors) = assign_joint_colors(&left_atomic, &right_atomic)?;

    let mut refinement_applications = 0usize;
    loop {
        let old_class_count = joint_color_class_count(&left_colors, &right_colors);
        let left_refined =
            refinement_signatures(&left_tuples, &left_colors, left.domain_size(), dimension)?;
        let right_refined =
            refinement_signatures(&right_tuples, &right_colors, right.domain_size(), dimension)?;
        let (next_left, next_right) = assign_joint_colors(&left_refined, &right_refined)?;
        refinement_applications = refinement_applications
            .checked_add(1)
            .ok_or(ObliviousWlError::RefinementCounterOverflow)?;
        let next_class_count = joint_color_class_count(&next_left, &next_right);

        left_colors = next_left;
        right_colors = next_right;

        if next_class_count == old_class_count {
            return Ok(ObliviousWlComparison {
                dimension,
                refinement_applications,
                distinguished: color_histogram(&left_colors) != color_histogram(&right_colors),
                stable_color_classes: next_class_count,
                left_colors,
                right_colors,
            });
        }
    }
}

fn ensure_tuple_buffer(dimension: usize) -> Result<(), ObliviousWlError> {
    let mut tuple = Vec::<u64>::new();
    tuple
        .try_reserve_exact(dimension)
        .map_err(|_| ObliviousWlError::TupleDimensionNotAddressable { dimension })?;
    Ok(())
}

fn checked_tuple_count(domain_size: u64, dimension: usize) -> Result<usize, ObliviousWlError> {
    let domain = usize::try_from(domain_size)
        .map_err(|_| ObliviousWlError::DomainNotAddressable { domain_size })?;
    let mut count = 1usize;
    for _ in 0..dimension {
        count = count
            .checked_mul(domain)
            .ok_or(ObliviousWlError::TupleSpaceNotAddressable {
                domain_size,
                dimension,
            })?;
    }
    Ok(count)
}

fn enumerate_tuples(
    domain_size: u64,
    dimension: usize,
    tuple_count: usize,
) -> Result<Vec<Vec<u64>>, ObliviousWlError> {
    let mut tuples = Vec::new();
    tuples.try_reserve_exact(tuple_count).map_err(|_| {
        ObliviousWlError::TupleSpaceNotAddressable {
            domain_size,
            dimension,
        }
    })?;

    let mut tuple = Vec::new();
    tuple
        .try_reserve_exact(dimension)
        .map_err(|_| ObliviousWlError::TupleDimensionNotAddressable { dimension })?;
    tuple.resize(dimension, 0);

    loop {
        tuples.push(tuple.clone());
        let mut position = dimension;
        loop {
            if position == 0 {
                if tuples.len() != tuple_count {
                    return Err(ObliviousWlError::TupleEnumerationInvariant {
                        expected: tuple_count,
                        actual: tuples.len(),
                    });
                }
                return Ok(tuples);
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

fn atomic_signatures(
    structure: &FiniteStructure,
    order: Option<&OrderedFiniteStructure>,
    tuples: &[Vec<u64>],
    dimension: usize,
) -> Result<Vec<AtomicSignature>, ObliviousWlError> {
    let mut signatures = Vec::new();
    signatures
        .try_reserve_exact(tuples.len())
        .map_err(|_| ObliviousWlError::SignatureBufferNotAddressable)?;

    for tuple in tuples {
        let mut bits = Vec::new();

        for left_index in 0..dimension {
            for right_index in 0..dimension {
                bits.push(tuple[left_index] == tuple[right_index]);
            }
        }

        for symbol in structure.vocabulary().relations() {
            let relation = structure
                .relation(symbol.name())
                .ok_or_else(|| ObliviousWlError::MissingRelation(symbol.name().to_owned()))?;
            let arity = usize::try_from(symbol.arity()).map_err(|_| {
                ObliviousWlError::ArityNotAddressable {
                    relation: symbol.name().to_owned(),
                    arity: symbol.arity(),
                }
            })?;
            for_each_coordinate_tuple(dimension, arity, |indices| {
                let mut relation_tuple = Vec::new();
                relation_tuple
                    .try_reserve_exact(arity)
                    .map_err(|_| ObliviousWlError::RelationTupleBufferNotAddressable { arity })?;
                relation_tuple.extend(indices.iter().map(|&index| tuple[index]));
                bits.push(relation.contains(&relation_tuple));
                Ok(())
            })?;
        }

        if let Some(order) = order {
            for left_index in 0..dimension {
                for right_index in 0..dimension {
                    bits.push(order.less_than(tuple[left_index], tuple[right_index]));
                }
            }
        }

        signatures.push(AtomicSignature(bits));
    }

    Ok(signatures)
}

fn for_each_coordinate_tuple<F>(
    dimension: usize,
    arity: usize,
    mut visit: F,
) -> Result<(), ObliviousWlError>
where
    F: FnMut(&[usize]) -> Result<(), ObliviousWlError>,
{
    if arity == 0 {
        return visit(&[]);
    }

    let mut indices = Vec::new();
    indices
        .try_reserve_exact(arity)
        .map_err(|_| ObliviousWlError::CoordinateTupleBufferNotAddressable { arity })?;
    indices.resize(arity, 0);

    loop {
        visit(&indices)?;
        let mut position = arity;
        loop {
            if position == 0 {
                return Ok(());
            }
            position -= 1;
            indices[position] += 1;
            if indices[position] < dimension {
                break;
            }
            indices[position] = 0;
        }
    }
}

fn refinement_signatures(
    tuples: &[Vec<u64>],
    colors: &[u64],
    domain_size: u64,
    dimension: usize,
) -> Result<Vec<RefinementSignature>, ObliviousWlError> {
    if tuples.len() != colors.len() {
        return Err(ObliviousWlError::ColorCardinalityInvariant {
            tuples: tuples.len(),
            colors: colors.len(),
        });
    }

    let domain = usize::try_from(domain_size)
        .map_err(|_| ObliviousWlError::DomainNotAddressable { domain_size })?;
    let mut signatures = Vec::new();
    signatures
        .try_reserve_exact(tuples.len())
        .map_err(|_| ObliviousWlError::SignatureBufferNotAddressable)?;

    for (tuple_index, tuple) in tuples.iter().enumerate() {
        let mut coordinate_multisets = Vec::new();
        coordinate_multisets
            .try_reserve_exact(dimension)
            .map_err(|_| ObliviousWlError::TupleDimensionNotAddressable { dimension })?;

        for coordinate in 0..dimension {
            let mut multiset = Vec::new();
            multiset
                .try_reserve_exact(domain)
                .map_err(|_| ObliviousWlError::NeighborhoodBufferNotAddressable { domain_size })?;
            for replacement in 0..domain_size {
                let mut neighbor = tuple.clone();
                neighbor[coordinate] = replacement;
                let neighbor_index = tuple_linear_index(&neighbor, domain_size)?;
                let color = colors.get(neighbor_index).copied().ok_or(
                    ObliviousWlError::TupleIndexInvariant {
                        index: neighbor_index,
                        tuple_count: colors.len(),
                    },
                )?;
                multiset.push(color);
            }
            multiset.sort_unstable();
            coordinate_multisets.push(multiset);
        }

        signatures.push(RefinementSignature {
            current: colors[tuple_index],
            coordinate_multisets,
        });
    }

    Ok(signatures)
}

fn tuple_linear_index(tuple: &[u64], domain_size: u64) -> Result<usize, ObliviousWlError> {
    let domain = usize::try_from(domain_size)
        .map_err(|_| ObliviousWlError::DomainNotAddressable { domain_size })?;
    let mut index = 0usize;
    for &value in tuple {
        let value = usize::try_from(value)
            .map_err(|_| ObliviousWlError::DomainNotAddressable { domain_size })?;
        index = index
            .checked_mul(domain)
            .and_then(|prefix| prefix.checked_add(value))
            .ok_or(ObliviousWlError::TupleIndexOverflow)?;
    }
    Ok(index)
}

fn assign_joint_colors<S: Ord + Clone>(
    left: &[S],
    right: &[S],
) -> Result<(Vec<u64>, Vec<u64>), ObliviousWlError> {
    let total = left
        .len()
        .checked_add(right.len())
        .ok_or(ObliviousWlError::SignatureCardinalityOverflow)?;
    let mut unique = Vec::new();
    unique
        .try_reserve_exact(total)
        .map_err(|_| ObliviousWlError::SignatureBufferNotAddressable)?;
    unique.extend_from_slice(left);
    unique.extend_from_slice(right);
    unique.sort();
    unique.dedup();

    let colorize = |signature: &S| -> Result<u64, ObliviousWlError> {
        let index = unique
            .binary_search(signature)
            .map_err(|_| ObliviousWlError::ColorAssignmentInvariant)?;
        u64::try_from(index).map_err(|_| ObliviousWlError::ColorSpaceNotAddressable)
    };

    let mut left_colors = Vec::new();
    left_colors
        .try_reserve_exact(left.len())
        .map_err(|_| ObliviousWlError::ColorBufferNotAddressable)?;
    for signature in left {
        left_colors.push(colorize(signature)?);
    }

    let mut right_colors = Vec::new();
    right_colors
        .try_reserve_exact(right.len())
        .map_err(|_| ObliviousWlError::ColorBufferNotAddressable)?;
    for signature in right {
        right_colors.push(colorize(signature)?);
    }

    Ok((left_colors, right_colors))
}

fn joint_color_class_count(left: &[u64], right: &[u64]) -> usize {
    let mut colors = left.to_vec();
    colors.extend_from_slice(right);
    colors.sort_unstable();
    colors.dedup();
    colors.len()
}

fn color_histogram(colors: &[u64]) -> BTreeMap<u64, usize> {
    let mut histogram = BTreeMap::new();
    for &color in colors {
        *histogram.entry(color).or_insert(0) += 1;
    }
    histogram
}

/// Fail-closed errors for exact coordinate-wise `k`-WL refinement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObliviousWlError {
    /// This implementation deliberately reserves `k`-WL for dimensions at least two.
    DimensionTooSmall(usize),
    /// Both structures must interpret exactly the same vocabulary.
    VocabularyMismatch,
    /// Ordered and unordered modes must not be mixed internally.
    OrderModeMismatch,
    /// The finite domain cannot be indexed by this platform.
    DomainNotAddressable { domain_size: u64 },
    /// A tuple of the requested dimension cannot be represented.
    TupleDimensionNotAddressable { dimension: usize },
    /// The explicit `domain_size^dimension` tuple space cannot be represented.
    TupleSpaceNotAddressable { domain_size: u64, dimension: usize },
    /// Tuple enumeration produced a count different from the prevalidated state space.
    TupleEnumerationInvariant { expected: usize, actual: usize },
    /// A vocabulary relation arity cannot be represented by this platform.
    ArityNotAddressable { relation: String, arity: u64 },
    /// A validated relation disappeared from a structure.
    MissingRelation(String),
    /// Coordinate-index tuples for an atomic relation cannot be represented.
    CoordinateTupleBufferNotAddressable { arity: usize },
    /// A concrete relation tuple buffer cannot be represented.
    RelationTupleBufferNotAddressable { arity: usize },
    /// Atomic/refinement signatures cannot be represented.
    SignatureBufferNotAddressable,
    /// The combined signature count overflowed.
    SignatureCardinalityOverflow,
    /// Stable/refinement color buffers cannot be represented.
    ColorBufferNotAddressable,
    /// A joint color identifier does not fit `u64`.
    ColorSpaceNotAddressable,
    /// A signature vanished during deterministic color assignment.
    ColorAssignmentInvariant,
    /// Tuple and color vectors lost their one-to-one correspondence.
    ColorCardinalityInvariant { tuples: usize, colors: usize },
    /// A coordinate-neighborhood multiset cannot be represented.
    NeighborhoodBufferNotAddressable { domain_size: u64 },
    /// A tuple-to-linear-index calculation overflowed.
    TupleIndexOverflow,
    /// A computed tuple index fell outside the explicit color vector.
    TupleIndexInvariant { index: usize, tuple_count: usize },
    /// The deterministic refinement-application counter overflowed.
    RefinementCounterOverflow,
}

impl fmt::Display for ObliviousWlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionTooSmall(dimension) => write!(
                formatter,
                "coordinate-wise k-WL requires dimension >= 2, found {dimension}"
            ),
            Self::VocabularyMismatch => {
                formatter.write_str("k-WL structures use different vocabularies")
            }
            Self::OrderModeMismatch => {
                formatter.write_str("k-WL mixed ordered and unordered modes")
            }
            Self::DomainNotAddressable { domain_size } => write!(
                formatter,
                "k-WL domain size {domain_size} cannot be indexed on this platform"
            ),
            Self::TupleDimensionNotAddressable { dimension } => write!(
                formatter,
                "k-WL tuple dimension {dimension} cannot be represented"
            ),
            Self::TupleSpaceNotAddressable {
                domain_size,
                dimension,
            } => write!(
                formatter,
                "k-WL tuple space {domain_size}^{dimension} cannot be represented"
            ),
            Self::TupleEnumerationInvariant { expected, actual } => write!(
                formatter,
                "k-WL tuple enumeration produced {actual} tuples, expected {expected}"
            ),
            Self::ArityNotAddressable { relation, arity } => write!(
                formatter,
                "relation {relation} has k-WL-unaddressable arity {arity}"
            ),
            Self::MissingRelation(name) => {
                write!(
                    formatter,
                    "validated relation {name} is missing from k-WL structure"
                )
            }
            Self::CoordinateTupleBufferNotAddressable { arity } => write!(
                formatter,
                "k-WL coordinate tuple of arity {arity} cannot be represented"
            ),
            Self::RelationTupleBufferNotAddressable { arity } => write!(
                formatter,
                "k-WL relation tuple of arity {arity} cannot be represented"
            ),
            Self::SignatureBufferNotAddressable => {
                formatter.write_str("k-WL signature buffer cannot be represented")
            }
            Self::SignatureCardinalityOverflow => {
                formatter.write_str("k-WL combined signature cardinality overflowed")
            }
            Self::ColorBufferNotAddressable => {
                formatter.write_str("k-WL color buffer cannot be represented")
            }
            Self::ColorSpaceNotAddressable => {
                formatter.write_str("k-WL color identifier cannot be represented as u64")
            }
            Self::ColorAssignmentInvariant => {
                formatter.write_str("k-WL signature vanished during color assignment")
            }
            Self::ColorCardinalityInvariant { tuples, colors } => {
                write!(formatter, "k-WL has {tuples} tuples but {colors} colors")
            }
            Self::NeighborhoodBufferNotAddressable { domain_size } => write!(
                formatter,
                "k-WL replacement multiset for domain size {domain_size} cannot be represented"
            ),
            Self::TupleIndexOverflow => formatter.write_str("k-WL tuple index overflowed"),
            Self::TupleIndexInvariant { index, tuple_count } => write!(
                formatter,
                "k-WL tuple index {index} is outside tuple count {tuple_count}"
            ),
            Self::RefinementCounterOverflow => {
                formatter.write_str("k-WL refinement counter overflowed")
            }
        }
    }
}

impl Error for ObliviousWlError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DescriptiveError, RelationInterpretation, RelationSymbol, Vocabulary};

    fn empty_structure(size: u64) -> FiniteStructure {
        FiniteStructure::new(size, Vocabulary::new(vec![]).unwrap(), vec![]).unwrap()
    }

    fn directed_graph(
        size: u64,
        edges: Vec<Vec<u64>>,
    ) -> Result<FiniteStructure, DescriptiveError> {
        let edge = RelationSymbol::new("E", 2)?;
        let vocabulary = Vocabulary::new(vec![edge.clone()])?;
        let relation = RelationInterpretation::new(edge, edges)?;
        FiniteStructure::new(size, vocabulary, vec![relation])
    }

    fn nullary_structure(value: bool) -> Result<FiniteStructure, DescriptiveError> {
        let flag = RelationSymbol::new("Q", 0)?;
        let vocabulary = Vocabulary::new(vec![flag.clone()])?;
        let tuples = if value { vec![vec![]] } else { vec![] };
        let relation = RelationInterpretation::new(flag, tuples)?;
        FiniteStructure::new(2, vocabulary, vec![relation])
    }

    #[test]
    fn dimension_one_is_rejected_by_convention() {
        let structure = empty_structure(2);
        assert_eq!(
            compare_oblivious_wl_unordered(&structure, &structure, 1),
            Err(ObliviousWlError::DimensionTooSmall(1))
        );
    }

    #[test]
    fn isomorphic_relabelled_graphs_are_not_distinguished() {
        let left = directed_graph(3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let right = directed_graph(3, vec![vec![2, 0], vec![0, 1]]).unwrap();

        let result = compare_oblivious_wl_unordered(&left, &right, 2).unwrap();
        assert!(!result.distinguished());
        assert!(result.refinement_applications() > 0);
        assert_eq!(result.left_colors().len(), 9);
        assert_eq!(result.right_colors().len(), 9);
    }

    #[test]
    fn unequal_cardinality_is_distinguished() {
        let two = empty_structure(2);
        let three = empty_structure(3);

        assert!(
            compare_oblivious_wl_unordered(&two, &three, 2)
                .unwrap()
                .distinguished()
        );
    }

    #[test]
    fn nullary_relation_difference_is_in_atomic_type() {
        let truth = nullary_structure(true).unwrap();
        let falsity = nullary_structure(false).unwrap();

        assert!(
            compare_oblivious_wl_unordered(&truth, &falsity, 2)
                .unwrap()
                .distinguished()
        );
    }

    #[test]
    fn isomorphic_distinguished_orders_are_not_distinguished() {
        let left = OrderedFiniteStructure::new(empty_structure(3), vec![0, 1, 2]).unwrap();
        let right = OrderedFiniteStructure::new(empty_structure(3), vec![2, 0, 1]).unwrap();

        assert!(
            !compare_oblivious_wl_ordered(&left, &right, 2)
                .unwrap()
                .distinguished()
        );
    }

    #[test]
    fn vocabulary_mismatch_fails_closed() {
        let empty = empty_structure(2);
        let graph = directed_graph(2, vec![]).unwrap();

        assert_eq!(
            compare_oblivious_wl_unordered(&empty, &graph, 2),
            Err(ObliviousWlError::VocabularyMismatch)
        );
    }

    #[test]
    fn impossible_dimension_fails_closed_before_iteration() {
        let structure = empty_structure(2);
        assert_eq!(
            compare_oblivious_wl_unordered(&structure, &structure, usize::MAX),
            Err(ObliviousWlError::TupleDimensionNotAddressable {
                dimension: usize::MAX,
            })
        );
    }

    #[test]
    fn comparison_is_deterministic() {
        let left = directed_graph(3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let right = directed_graph(3, vec![vec![2, 0], vec![0, 1]]).unwrap();

        let first = compare_oblivious_wl_unordered(&left, &right, 2).unwrap();
        let second = compare_oblivious_wl_unordered(&left, &right, 2).unwrap();
        assert_eq!(first, second);
        assert!(first.stable_color_classes() > 0);
    }
}
