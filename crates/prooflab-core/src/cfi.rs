//! Deterministic Cai–Fürer–Immerman calibration metadata for PL-DC.
//!
//! This module deliberately stops before constructing CFI gadgets. It provides
//! an exact, auditable representation of a finite undirected base graph and a
//! twist bit on every base edge, together with the global twist parity used by
//! later calibration fixtures. These values are experimental/search inputs,
//! never proof objects and never evidence for `P != NP`.

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

/// Fail-closed CFI calibration-input validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfiError {
    SelfLoop(u32),
    EndpointOutOfRange { endpoint: u32, vertex_count: u32 },
    DuplicateEdge(CfiBaseEdge),
    TwistCardinalityMismatch { edges: usize, twists: usize },
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
        }
    }
}

impl Error for CfiError {}

#[cfg(test)]
mod tests {
    use super::*;

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
}
