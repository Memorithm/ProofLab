//! Deterministic Cai–Fürer–Immerman calibration instances.
//!
//! This first generator deliberately targets connected simple 3-regular base
//! graphs. For each base vertex `v`, the CFI gadget contains the four even bit
//! vectors in `F_2^3`. For a canonical base edge `{u,v}`, the generated graph
//! connects gadget vertices `(u,a)` and `(v,b)` exactly when the two local bits
//! corresponding to that base edge satisfy `a_i XOR b_j = twist(edge)`.
//!
//! Vertices are additionally partitioned by unary base-vertex colors. This is a
//! finite colored CFI structure for calibration of game/WL oracles; no lower
//! bound or complexity-class separation is inferred from generating it.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    Canonical, CanonicalEncoder, DescriptiveError, FiniteStructure, RelationInterpretation,
    RelationSymbol, Vocabulary,
};

const CFI_LOCAL_PATTERNS: [[bool; 3]; 4] = [
    [false, false, false],
    [false, true, true],
    [true, false, true],
    [true, true, false],
];
const CFI_VERTICES_PER_BASE_VERTEX: u64 = 4;

/// Canonical connected simple cubic base graph for the first CFI calibration family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiBaseGraph {
    vertex_count: u64,
    edges: Vec<(u64, u64)>,
    neighbors: Vec<Vec<u64>>,
}

impl CfiBaseGraph {
    /// Construct and validate a connected simple 3-regular graph.
    ///
    /// Input edges are normalized to `(min, max)` and sorted lexicographically.
    ///
    /// # Errors
    ///
    /// Returns [`CfiError`] for an empty/unaddressable carrier, self-loops,
    /// out-of-domain or duplicate edges, non-cubic degree, or disconnected input.
    pub fn new(vertex_count: u64, edges: Vec<(u64, u64)>) -> Result<Self, CfiError> {
        if vertex_count == 0 {
            return Err(CfiError::EmptyBaseGraph);
        }
        let count = usize::try_from(vertex_count)
            .map_err(|_| CfiError::BaseDomainNotAddressable { vertex_count })?;

        let mut canonical_edges = Vec::new();
        canonical_edges
            .try_reserve_exact(edges.len())
            .map_err(|_| CfiError::EdgeBufferNotAddressable)?;
        for (left, right) in edges {
            if left >= vertex_count {
                return Err(CfiError::BaseVertexOutOfDomain {
                    vertex: left,
                    vertex_count,
                });
            }
            if right >= vertex_count {
                return Err(CfiError::BaseVertexOutOfDomain {
                    vertex: right,
                    vertex_count,
                });
            }
            if left == right {
                return Err(CfiError::SelfLoop(left));
            }
            canonical_edges.push(if left < right {
                (left, right)
            } else {
                (right, left)
            });
        }
        canonical_edges.sort_unstable();
        for pair in canonical_edges.windows(2) {
            if pair[0] == pair[1] {
                return Err(CfiError::DuplicateEdge(pair[0]));
            }
        }

        let mut neighbors = Vec::new();
        neighbors
            .try_reserve_exact(count)
            .map_err(|_| CfiError::NeighborBufferNotAddressable { vertex_count })?;
        neighbors.resize_with(count, Vec::new);
        for &(left, right) in &canonical_edges {
            let left_index = usize::try_from(left)
                .map_err(|_| CfiError::BaseDomainNotAddressable { vertex_count })?;
            let right_index = usize::try_from(right)
                .map_err(|_| CfiError::BaseDomainNotAddressable { vertex_count })?;
            neighbors[left_index].push(right);
            neighbors[right_index].push(left);
        }
        for (vertex, adjacent) in neighbors.iter_mut().enumerate() {
            adjacent.sort_unstable();
            if adjacent.len() != 3 {
                return Err(CfiError::NonCubicDegree {
                    vertex: u64::try_from(vertex)
                        .map_err(|_| CfiError::BaseDomainNotAddressable { vertex_count })?,
                    degree: adjacent.len(),
                });
            }
        }

        if !is_connected(&neighbors)? {
            return Err(CfiError::DisconnectedBaseGraph);
        }

        Ok(Self {
            vertex_count,
            edges: canonical_edges,
            neighbors,
        })
    }

    /// Number of base vertices.
    #[must_use]
    pub const fn vertex_count(&self) -> u64 {
        self.vertex_count
    }

    /// Canonical undirected base edges in lexicographic order.
    #[must_use]
    pub fn edges(&self) -> &[(u64, u64)] {
        &self.edges
    }

    /// Canonically sorted neighbors of one base vertex.
    #[must_use]
    pub fn neighbors(&self, vertex: u64) -> Option<&[u64]> {
        usize::try_from(vertex)
            .ok()
            .and_then(|index| self.neighbors.get(index))
            .map(Vec::as_slice)
    }
}

impl Canonical for CfiBaseGraph {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str("prooflab.descriptive.CfiBaseGraph/v1");
        encoder.u64(self.vertex_count);
        encoder.u64(self.edges.len() as u64);
        for &(left, right) in &self.edges {
            encoder.u64(left);
            encoder.u64(right);
        }
    }
}

/// One deterministic colored CFI instance over a validated cubic base graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiInstance {
    base: CfiBaseGraph,
    twist_bits: Vec<bool>,
    twist_parity: bool,
    structure: FiniteStructure,
}

impl CfiInstance {
    /// Underlying canonical cubic base graph.
    #[must_use]
    pub const fn base(&self) -> &CfiBaseGraph {
        &self.base
    }

    /// Twist bits in canonical base-edge order.
    #[must_use]
    pub fn twist_bits(&self) -> &[bool] {
        &self.twist_bits
    }

    /// XOR parity of all canonical edge twists.
    #[must_use]
    pub const fn twist_parity(&self) -> bool {
        self.twist_parity
    }

    /// Generated colored finite relational structure.
    #[must_use]
    pub const fn structure(&self) -> &FiniteStructure {
        &self.structure
    }
}

/// Build a deterministic colored CFI graph from a cubic base and edge-twist vector.
///
/// # Errors
///
/// Returns [`CfiError`] if `twist_bits` does not match canonical edge count,
/// generated sizes cannot be represented, an internal base-edge coordinate is
/// inconsistent, or the final finite structure fails validation.
pub fn build_cfi(base: &CfiBaseGraph, twist_bits: &[bool]) -> Result<CfiInstance, CfiError> {
    if twist_bits.len() != base.edges.len() {
        return Err(CfiError::TwistCardinality {
            expected: base.edges.len(),
            actual: twist_bits.len(),
        });
    }

    let cfi_domain_size = base
        .vertex_count
        .checked_mul(CFI_VERTICES_PER_BASE_VERTEX)
        .ok_or(CfiError::GeneratedDomainOverflow {
            vertex_count: base.vertex_count,
        })?;

    let adjacency_symbol = RelationSymbol::new("cfi:adjacency", 2)?;
    let mut symbols = Vec::new();
    symbols
        .try_reserve_exact(
            usize::try_from(base.vertex_count)
                .map_err(|_| CfiError::BaseDomainNotAddressable {
                    vertex_count: base.vertex_count,
                })?
                .saturating_add(1),
        )
        .map_err(|_| CfiError::VocabularyBufferNotAddressable)?;
    symbols.push(adjacency_symbol.clone());
    for vertex in 0..base.vertex_count {
        symbols.push(RelationSymbol::new(color_name(vertex), 1)?);
    }
    let vocabulary = Vocabulary::new(symbols)?;

    let mut adjacency = Vec::new();
    for (edge_index, &(left, right)) in base.edges.iter().enumerate() {
        let left_coordinate = neighbor_coordinate(base, left, right)?;
        let right_coordinate = neighbor_coordinate(base, right, left)?;
        let twist = twist_bits[edge_index];

        for (left_local, left_pattern) in CFI_LOCAL_PATTERNS.iter().enumerate() {
            for (right_local, right_pattern) in CFI_LOCAL_PATTERNS.iter().enumerate() {
                if left_pattern[left_coordinate] ^ right_pattern[right_coordinate] == twist {
                    let left_vertex = generated_vertex(left, left_local)?;
                    let right_vertex = generated_vertex(right, right_local)?;
                    adjacency.push(vec![left_vertex, right_vertex]);
                    adjacency.push(vec![right_vertex, left_vertex]);
                }
            }
        }
    }

    let mut interpretations = Vec::new();
    interpretations
        .try_reserve_exact(vocabulary.relations().len())
        .map_err(|_| CfiError::InterpretationBufferNotAddressable)?;
    interpretations.push(RelationInterpretation::new(
        adjacency_symbol,
        adjacency,
    )?);

    for vertex in 0..base.vertex_count {
        let symbol = RelationSymbol::new(color_name(vertex), 1)?;
        let mut tuples = Vec::new();
        tuples
            .try_reserve_exact(CFI_LOCAL_PATTERNS.len())
            .map_err(|_| CfiError::ColorBufferNotAddressable { vertex })?;
        for local in 0..CFI_LOCAL_PATTERNS.len() {
            tuples.push(vec![generated_vertex(vertex, local)?]);
        }
        interpretations.push(RelationInterpretation::new(symbol, tuples)?);
    }

    let structure = FiniteStructure::new(cfi_domain_size, vocabulary, interpretations)?;
    let twist_parity = twist_bits.iter().fold(false, |parity, &bit| parity ^ bit);

    Ok(CfiInstance {
        base: base.clone(),
        twist_bits: twist_bits.to_vec(),
        twist_parity,
        structure,
    })
}

fn color_name(vertex: u64) -> String {
    format!("cfi:base-color:{vertex:020}")
}

fn generated_vertex(base_vertex: u64, local: usize) -> Result<u64, CfiError> {
    let local = u64::try_from(local).map_err(|_| CfiError::LocalIndexInvariant { local })?;
    base_vertex
        .checked_mul(CFI_VERTICES_PER_BASE_VERTEX)
        .and_then(|offset| offset.checked_add(local))
        .ok_or(CfiError::GeneratedVertexOverflow { base_vertex, local })
}

fn neighbor_coordinate(base: &CfiBaseGraph, vertex: u64, neighbor: u64) -> Result<usize, CfiError> {
    let neighbors = base
        .neighbors(vertex)
        .ok_or(CfiError::MissingNeighborList { vertex })?;
    neighbors
        .binary_search(&neighbor)
        .map_err(|_| CfiError::MissingBaseIncidence { vertex, neighbor })
}

fn is_connected(neighbors: &[Vec<u64>]) -> Result<bool, CfiError> {
    let mut seen = Vec::new();
    seen.try_reserve_exact(neighbors.len())
        .map_err(|_| CfiError::ConnectivityBufferNotAddressable)?;
    seen.resize(neighbors.len(), false);

    let mut stack = Vec::new();
    stack
        .try_reserve_exact(neighbors.len())
        .map_err(|_| CfiError::ConnectivityBufferNotAddressable)?;
    seen[0] = true;
    stack.push(0usize);

    while let Some(vertex) = stack.pop() {
        for &neighbor in &neighbors[vertex] {
            let index = usize::try_from(neighbor)
                .map_err(|_| CfiError::ConnectivityIndexInvariant { neighbor })?;
            if !seen[index] {
                seen[index] = true;
                stack.push(index);
            }
        }
    }

    Ok(seen.into_iter().all(|visited| visited))
}

/// Validation and materialization failures for deterministic cubic CFI instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfiError {
    EmptyBaseGraph,
    BaseDomainNotAddressable { vertex_count: u64 },
    EdgeBufferNotAddressable,
    BaseVertexOutOfDomain { vertex: u64, vertex_count: u64 },
    SelfLoop(u64),
    DuplicateEdge((u64, u64)),
    NeighborBufferNotAddressable { vertex_count: u64 },
    NonCubicDegree { vertex: u64, degree: usize },
    DisconnectedBaseGraph,
    ConnectivityBufferNotAddressable,
    ConnectivityIndexInvariant { neighbor: u64 },
    TwistCardinality { expected: usize, actual: usize },
    GeneratedDomainOverflow { vertex_count: u64 },
    VocabularyBufferNotAddressable,
    InterpretationBufferNotAddressable,
    ColorBufferNotAddressable { vertex: u64 },
    MissingNeighborList { vertex: u64 },
    MissingBaseIncidence { vertex: u64, neighbor: u64 },
    LocalIndexInvariant { local: usize },
    GeneratedVertexOverflow { base_vertex: u64, local: u64 },
    Descriptive(DescriptiveError),
}

impl From<DescriptiveError> for CfiError {
    fn from(error: DescriptiveError) -> Self {
        Self::Descriptive(error)
    }
}

impl fmt::Display for CfiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBaseGraph => formatter.write_str("CFI base graph must be non-empty"),
            Self::BaseDomainNotAddressable { vertex_count } => write!(
                formatter,
                "CFI base carrier of size {vertex_count} cannot be indexed"
            ),
            Self::EdgeBufferNotAddressable => formatter.write_str("CFI base edge buffer cannot be represented"),
            Self::BaseVertexOutOfDomain {
                vertex,
                vertex_count,
            } => write!(
                formatter,
                "CFI base vertex {vertex} is outside carrier size {vertex_count}"
            ),
            Self::SelfLoop(vertex) => write!(formatter, "CFI base graph has self-loop at {vertex}"),
            Self::DuplicateEdge(edge) => write!(formatter, "CFI base graph repeats edge {edge:?}"),
            Self::NeighborBufferNotAddressable { vertex_count } => write!(
                formatter,
                "CFI neighbor table for {vertex_count} vertices cannot be represented"
            ),
            Self::NonCubicDegree { vertex, degree } => write!(
                formatter,
                "CFI base vertex {vertex} has degree {degree}; expected exactly 3"
            ),
            Self::DisconnectedBaseGraph => formatter.write_str("CFI base graph must be connected"),
            Self::ConnectivityBufferNotAddressable => {
                formatter.write_str("CFI connectivity workspace cannot be represented")
            }
            Self::ConnectivityIndexInvariant { neighbor } => write!(
                formatter,
                "CFI connectivity neighbor {neighbor} cannot be indexed"
            ),
            Self::TwistCardinality { expected, actual } => write!(
                formatter,
                "CFI twist vector has length {actual}; expected {expected} canonical edges"
            ),
            Self::GeneratedDomainOverflow { vertex_count } => write!(
                formatter,
                "CFI domain size overflow for {vertex_count} base vertices"
            ),
            Self::VocabularyBufferNotAddressable => {
                formatter.write_str("CFI vocabulary buffer cannot be represented")
            }
            Self::InterpretationBufferNotAddressable => {
                formatter.write_str("CFI interpretation buffer cannot be represented")
            }
            Self::ColorBufferNotAddressable { vertex } => write!(
                formatter,
                "CFI color class buffer for base vertex {vertex} cannot be represented"
            ),
            Self::MissingNeighborList { vertex } => {
                write!(formatter, "CFI base vertex {vertex} has no neighbor list")
            }
            Self::MissingBaseIncidence { vertex, neighbor } => write!(
                formatter,
                "CFI base incidence {vertex}--{neighbor} is missing from canonical neighbor order"
            ),
            Self::LocalIndexInvariant { local } => write!(
                formatter,
                "CFI local gadget index {local} cannot be converted to u64"
            ),
            Self::GeneratedVertexOverflow { base_vertex, local } => write!(
                formatter,
                "CFI generated vertex overflow at base vertex {base_vertex}, local index {local}"
            ),
            Self::Descriptive(error) => write!(formatter, "invalid generated CFI structure: {error}"),
        }
    }
}

impl Error for CfiError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn k4_edges() -> Vec<(u64, u64)> {
        vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]
    }

    #[test]
    fn k4_base_is_canonical_and_cubic() {
        let base = CfiBaseGraph::new(
            4,
            vec![(3, 2), (1, 0), (3, 0), (2, 1), (3, 1), (2, 0)],
        )
        .unwrap();
        assert_eq!(base.edges(), k4_edges());
        for vertex in 0..4 {
            assert_eq!(base.neighbors(vertex).unwrap().len(), 3);
        }
    }

    #[test]
    fn k4_cfi_has_expected_colored_domain_and_adjacency_size() {
        let base = CfiBaseGraph::new(4, k4_edges()).unwrap();
        let instance = build_cfi(&base, &[false; 6]).unwrap();
        let structure = instance.structure();

        assert_eq!(structure.domain_size(), 16);
        assert_eq!(structure.vocabulary().relations().len(), 5);
        assert_eq!(structure.relation("cfi:adjacency").unwrap().tuples().len(), 96);
        for vertex in 0..4 {
            assert_eq!(
                structure
                    .relation(&color_name(vertex))
                    .unwrap()
                    .tuples()
                    .len(),
                4
            );
        }
    }

    #[test]
    fn generated_adjacency_is_symmetric_and_loop_free() {
        let base = CfiBaseGraph::new(4, k4_edges()).unwrap();
        let instance = build_cfi(&base, &[true, false, false, false, false, false]).unwrap();
        let adjacency = instance.structure().relation("cfi:adjacency").unwrap();

        for tuple in adjacency.tuples() {
            assert_ne!(tuple[0], tuple[1]);
            assert!(adjacency.contains(&[tuple[1], tuple[0]]));
        }
    }

    #[test]
    fn twist_parity_tracks_canonical_edge_bits() {
        let base = CfiBaseGraph::new(4, k4_edges()).unwrap();
        assert!(!build_cfi(&base, &[false; 6]).unwrap().twist_parity());
        assert!(
            build_cfi(&base, &[true, false, false, false, false, false])
                .unwrap()
                .twist_parity()
        );
        assert!(
            !build_cfi(&base, &[true, true, false, false, false, false])
                .unwrap()
                .twist_parity()
        );
    }

    #[test]
    fn twist_length_must_match_canonical_edges() {
        let base = CfiBaseGraph::new(4, k4_edges()).unwrap();
        assert_eq!(
            build_cfi(&base, &[false; 5]),
            Err(CfiError::TwistCardinality {
                expected: 6,
                actual: 5,
            })
        );
    }

    #[test]
    fn non_cubic_base_is_rejected() {
        assert_eq!(
            CfiBaseGraph::new(3, vec![(0, 1), (1, 2), (0, 2)]),
            Err(CfiError::NonCubicDegree {
                vertex: 0,
                degree: 2,
            })
        );
    }

    #[test]
    fn disconnected_cubic_base_is_rejected() {
        let mut edges = k4_edges();
        edges.extend(
            k4_edges()
                .into_iter()
                .map(|(left, right)| (left + 4, right + 4)),
        );
        assert_eq!(
            CfiBaseGraph::new(8, edges),
            Err(CfiError::DisconnectedBaseGraph)
        );
    }

    #[test]
    fn duplicate_base_edge_is_rejected_after_normalization() {
        let mut edges = k4_edges();
        edges.push((1, 0));
        assert_eq!(
            CfiBaseGraph::new(4, edges),
            Err(CfiError::DuplicateEdge((0, 1)))
        );
    }
}
