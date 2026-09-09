//! Deterministic Cai–Fürer–Immerman calibration infrastructure for PL-DC.
//!
//! This module provides an exact, auditable representation of a finite
//! undirected base graph, one twist bit per base edge, and a deliberately small
//! cubic CFI graph constructor. The constructor is calibration infrastructure:
//! generated finite graphs are experimental/search inputs, never proof objects
//! and never evidence for `P != NP`.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

/// Canonical undirected base edge, stored with `left < right`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CfiBaseEdge {
    left: u32,
    right: u32,
}

impl CfiBaseEdge {
    /// Construct one loop-free undirected edge in canonical endpoint order.
    ///
    /// # Errors
    ///
    /// Returns [`CfiError::SelfLoop`] when both endpoints are equal.
    pub fn new(a: u32, b: u32) -> Result<Self, CfiError> {
        if a == b {
            return Err(CfiError::SelfLoop(a));
        }
        let (left, right) = if a < b { (a, b) } else { (b, a) };
        Ok(Self { left, right })
    }

    /// Smaller endpoint.
    #[must_use]
    pub const fn left(self) -> u32 {
        self.left
    }

    /// Larger endpoint.
    #[must_use]
    pub const fn right(self) -> u32 {
        self.right
    }
}

/// Validated finite simple base graph for deterministic CFI calibration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiBaseGraph {
    vertex_count: u32,
    edges: Vec<CfiBaseEdge>,
}

impl CfiBaseGraph {
    /// Build a simple graph from edges; input order and endpoint orientation do
    /// not affect the stored representation.
    ///
    /// # Errors
    ///
    /// Fails on loops, endpoints outside `0..vertex_count`, or duplicate edges.
    pub fn new(vertex_count: u32, raw_edges: &[(u32, u32)]) -> Result<Self, CfiError> {
        let mut seen = BTreeSet::new();
        for &(a, b) in raw_edges {
            let edge = CfiBaseEdge::new(a, b)?;
            if edge.right >= vertex_count {
                return Err(CfiError::EndpointOutOfRange {
                    endpoint: edge.right,
                    vertex_count,
                });
            }
            if !seen.insert(edge) {
                return Err(CfiError::DuplicateEdge(edge));
            }
        }
        Ok(Self {
            vertex_count,
            edges: seen.into_iter().collect(),
        })
    }

    /// Number of base vertices.
    #[must_use]
    pub const fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Canonically sorted base edges.
    #[must_use]
    pub fn edges(&self) -> &[CfiBaseEdge] {
        &self.edges
    }
}

/// A twist assignment bound positionally to a canonical base-graph edge list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiTwistAssignment {
    bits: Vec<bool>,
}

impl CfiTwistAssignment {
    /// Bind exact twist bits to the canonical edges of `base`.
    ///
    /// # Errors
    ///
    /// Returns [`CfiError::TwistCardinalityMismatch`] unless there is exactly
    /// one twist bit per canonical base edge.
    pub fn new(base: &CfiBaseGraph, bits: Vec<bool>) -> Result<Self, CfiError> {
        if bits.len() != base.edges.len() {
            return Err(CfiError::TwistCardinalityMismatch {
                edges: base.edges.len(),
                twists: bits.len(),
            });
        }
        Ok(Self { bits })
    }

    /// Exact twist bits in canonical edge order.
    #[must_use]
    pub fn bits(&self) -> &[bool] {
        &self.bits
    }

    /// Parity of the number of twisted base edges (`false` even, `true` odd).
    #[must_use]
    pub fn odd_parity(&self) -> bool {
        self.bits.iter().fold(false, |parity, bit| parity ^ bit)
    }

    /// Number of twisted base edges.
    #[must_use]
    pub fn twist_count(&self) -> usize {
        self.bits.iter().filter(|&&bit| bit).count()
    }
}

/// Side of one link-vertex pair in a CFI gadget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CfiLinkSide {
    A,
    B,
}

/// Semantic identity of a vertex in the deterministic cubic CFI expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CfiVertex {
    /// One of the two link vertices associated with an incident base edge.
    Link {
        base_vertex: u32,
        slot: u8,
        side: CfiLinkSide,
    },
    /// A middle vertex indexed by an even subset of the three incident slots.
    Middle { base_vertex: u32, even_mask: u8 },
}

/// Deterministic CFI expansion for a cubic base graph.
///
/// Each base vertex contributes six link vertices and four middle vertices,
/// one for each even subset of its three incident edges. Each base edge then
/// contributes two parallel or crossed link edges according to its twist bit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubicCfiGraph {
    vertices: Vec<CfiVertex>,
    edges: Vec<(u32, u32)>,
}

impl CubicCfiGraph {
    /// Construct the standard degree-three CFI calibration graph.
    ///
    /// # Errors
    ///
    /// Fails unless every base vertex has degree exactly three, unless the
    /// twist assignment matches the base edge set, or if the expanded vertex
    /// count cannot be represented by this bootstrap implementation.
    pub fn new(base: &CfiBaseGraph, twists: &CfiTwistAssignment) -> Result<Self, CfiError> {
        if twists.bits.len() != base.edges.len() {
            return Err(CfiError::TwistCardinalityMismatch {
                edges: base.edges.len(),
                twists: twists.bits.len(),
            });
        }

        let vertex_count =
            usize::try_from(base.vertex_count).map_err(|_| CfiError::ExpandedGraphTooLarge)?;
        let mut incident = vec![Vec::<u32>::new(); vertex_count];
        for edge in &base.edges {
            incident[edge.left as usize].push(edge.right);
            incident[edge.right as usize].push(edge.left);
        }
        for (vertex, neighbours) in incident.iter_mut().enumerate() {
            neighbours.sort_unstable();
            if neighbours.len() != 3 {
                let vertex =
                    u32::try_from(vertex).map_err(|_| CfiError::InternalConstructionInvariant)?;
                return Err(CfiError::NonCubicVertex {
                    vertex,
                    degree: neighbours.len(),
                });
            }
        }

        let expanded_vertex_count = base
            .vertex_count
            .checked_mul(10)
            .ok_or(CfiError::ExpandedGraphTooLarge)?;
        let mut vertices = Vec::with_capacity(expanded_vertex_count as usize);
        for base_vertex in 0..base.vertex_count {
            for slot in 0_u8..3 {
                vertices.push(CfiVertex::Link {
                    base_vertex,
                    slot,
                    side: CfiLinkSide::A,
                });
                vertices.push(CfiVertex::Link {
                    base_vertex,
                    slot,
                    side: CfiLinkSide::B,
                });
            }
            for even_mask in [0_u8, 0b011, 0b101, 0b110] {
                vertices.push(CfiVertex::Middle {
                    base_vertex,
                    even_mask,
                });
            }
        }

        let mut edges = BTreeSet::new();
        for base_vertex in 0..base.vertex_count {
            let block = base_vertex * 10;
            for (middle_offset, even_mask) in [0_u8, 0b011, 0b101, 0b110].into_iter().enumerate() {
                let middle_offset = u32::try_from(middle_offset)
                    .map_err(|_| CfiError::InternalConstructionInvariant)?;
                let middle = block + 6 + middle_offset;
                for slot in 0_u8..3 {
                    let side_offset = u32::from(even_mask & (1 << slot) == 0);
                    let link = block + u32::from(slot) * 2 + side_offset;
                    edges.insert(canonical_pair(middle, link));
                }
            }
        }

        for (edge_index, edge) in base.edges.iter().enumerate() {
            let left_slot = incident[edge.left as usize]
                .binary_search(&edge.right)
                .map_err(|_| CfiError::InternalConstructionInvariant)?;
            let left_slot =
                u32::try_from(left_slot).map_err(|_| CfiError::InternalConstructionInvariant)?;
            let right_slot = incident[edge.right as usize]
                .binary_search(&edge.left)
                .map_err(|_| CfiError::InternalConstructionInvariant)?;
            let right_slot =
                u32::try_from(right_slot).map_err(|_| CfiError::InternalConstructionInvariant)?;
            let left_a = edge.left * 10 + left_slot * 2;
            let left_b = left_a + 1;
            let right_a = edge.right * 10 + right_slot * 2;
            let right_b = right_a + 1;
            if twists.bits[edge_index] {
                edges.insert(canonical_pair(left_a, right_b));
                edges.insert(canonical_pair(left_b, right_a));
            } else {
                edges.insert(canonical_pair(left_a, right_a));
                edges.insert(canonical_pair(left_b, right_b));
            }
        }

        Ok(Self {
            vertices,
            edges: edges.into_iter().collect(),
        })
    }

    /// Semantic vertices in deterministic numeric-id order.
    #[must_use]
    pub fn vertices(&self) -> &[CfiVertex] {
        &self.vertices
    }

    /// Canonical undirected expanded edges sorted lexicographically.
    #[must_use]
    pub fn edges(&self) -> &[(u32, u32)] {
        &self.edges
    }

    /// Number of expanded vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Number of expanded edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

fn canonical_pair(a: u32, b: u32) -> (u32, u32) {
    if a < b { (a, b) } else { (b, a) }
}

/// Fail-closed CFI calibration-input or construction validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfiError {
    SelfLoop(u32),
    EndpointOutOfRange { endpoint: u32, vertex_count: u32 },
    DuplicateEdge(CfiBaseEdge),
    TwistCardinalityMismatch { edges: usize, twists: usize },
    NonCubicVertex { vertex: u32, degree: usize },
    ExpandedGraphTooLarge,
    InternalConstructionInvariant,
}

impl fmt::Display for CfiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfLoop(vertex) => {
                write!(formatter, "CFI base graph contains self-loop at {vertex}")
            }
            Self::EndpointOutOfRange {
                endpoint,
                vertex_count,
            } => write!(
                formatter,
                "CFI base endpoint {endpoint} is outside carrier size {vertex_count}"
            ),
            Self::DuplicateEdge(edge) => write!(
                formatter,
                "CFI base graph repeats edge ({}, {})",
                edge.left, edge.right
            ),
            Self::TwistCardinalityMismatch { edges, twists } => write!(
                formatter,
                "CFI twist assignment has {twists} bits for {edges} base edges"
            ),
            Self::NonCubicVertex { vertex, degree } => write!(
                formatter,
                "CFI cubic calibration requires degree 3, vertex {vertex} has degree {degree}"
            ),
            Self::ExpandedGraphTooLarge => {
                write!(
                    formatter,
                    "CFI expanded graph exceeds bootstrap size limits"
                )
            }
            Self::InternalConstructionInvariant => {
                write!(
                    formatter,
                    "CFI construction violated an internal incidence invariant"
                )
            }
        }
    }
}

impl Error for CfiError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn k4_base() -> CfiBaseGraph {
        CfiBaseGraph::new(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]).unwrap()
    }

    #[test]
    fn base_graph_is_canonical_under_edge_order_and_orientation() {
        let a = CfiBaseGraph::new(4, &[(2, 0), (3, 2), (1, 0)]).unwrap();
        let b = CfiBaseGraph::new(4, &[(0, 1), (0, 2), (2, 3)]).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a.edges(),
            &[
                CfiBaseEdge::new(0, 1).unwrap(),
                CfiBaseEdge::new(0, 2).unwrap(),
                CfiBaseEdge::new(2, 3).unwrap(),
            ]
        );
    }

    #[test]
    fn invalid_base_graphs_fail_closed() {
        assert_eq!(CfiBaseGraph::new(2, &[(0, 0)]), Err(CfiError::SelfLoop(0)));
        assert_eq!(
            CfiBaseGraph::new(2, &[(0, 2)]),
            Err(CfiError::EndpointOutOfRange {
                endpoint: 2,
                vertex_count: 2,
            })
        );
        let edge = CfiBaseEdge::new(0, 1).unwrap();
        assert_eq!(
            CfiBaseGraph::new(2, &[(0, 1), (1, 0)]),
            Err(CfiError::DuplicateEdge(edge))
        );
    }

    #[test]
    fn twist_assignment_has_exact_reproducible_parity() {
        let base = CfiBaseGraph::new(3, &[(0, 1), (1, 2), (0, 2)]).unwrap();
        let even = CfiTwistAssignment::new(&base, vec![true, true, false]).unwrap();
        let odd = CfiTwistAssignment::new(&base, vec![true, false, false]).unwrap();
        assert!(!even.odd_parity());
        assert_eq!(even.twist_count(), 2);
        assert!(odd.odd_parity());
        assert_eq!(odd.twist_count(), 1);
    }

    #[test]
    fn twist_cardinality_mismatch_is_rejected() {
        let base = CfiBaseGraph::new(3, &[(0, 1), (1, 2)]).unwrap();
        assert_eq!(
            CfiTwistAssignment::new(&base, vec![true]),
            Err(CfiError::TwistCardinalityMismatch {
                edges: 2,
                twists: 1,
            })
        );
    }

    #[test]
    fn cubic_k4_expands_to_standard_ten_vertex_gadgets() {
        let base = k4_base();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let expanded = CubicCfiGraph::new(&base, &twists).unwrap();
        assert_eq!(expanded.vertex_count(), 40);
        assert_eq!(expanded.edge_count(), 60);
        assert_eq!(expanded.vertices().len(), 40);

        let mut degree = vec![0_u8; expanded.vertex_count()];
        for &(left, right) in expanded.edges() {
            degree[left as usize] += 1;
            degree[right as usize] += 1;
        }
        assert!(degree.into_iter().all(|value| value == 3));
    }

    #[test]
    fn one_twist_crosses_exactly_one_base_edge_pair() {
        let base = k4_base();
        let plain = CubicCfiGraph::new(
            &base,
            &CfiTwistAssignment::new(&base, vec![false; 6]).unwrap(),
        )
        .unwrap();
        let mut bits = vec![false; 6];
        bits[0] = true;
        let twisted =
            CubicCfiGraph::new(&base, &CfiTwistAssignment::new(&base, bits).unwrap()).unwrap();

        let plain_edges: BTreeSet<_> = plain.edges().iter().copied().collect();
        let twisted_edges: BTreeSet<_> = twisted.edges().iter().copied().collect();
        assert_eq!(plain_edges.symmetric_difference(&twisted_edges).count(), 4);
        assert_eq!(plain.vertex_count(), twisted.vertex_count());
        assert_eq!(plain.edge_count(), twisted.edge_count());
    }

    #[test]
    fn non_cubic_base_is_rejected_before_expansion() {
        let base = CfiBaseGraph::new(3, &[(0, 1), (1, 2)]).unwrap();
        let twists = CfiTwistAssignment::new(&base, vec![false; 2]).unwrap();
        assert_eq!(
            CubicCfiGraph::new(&base, &twists),
            Err(CfiError::NonCubicVertex {
                vertex: 0,
                degree: 1,
            })
        );
    }
}
