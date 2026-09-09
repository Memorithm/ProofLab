//! Auditable bridge from finite CFI calibration graphs into the PL-DC relational substrate.
//!
//! This module does not prove any CFI indistinguishability statement. It only
//! translates the deterministic undirected graph produced by [`CubicCfiGraph`]
//! into the generic [`FiniteStructure`] representation used by the exact finite
//! model-comparison oracles.

use crate::{
    CubicCfiGraph, DescriptiveError, FiniteStructure, RelationInterpretation, RelationSymbol,
    Vocabulary,
};

/// Canonical binary edge-relation name used by the CFI calibration bridge.
pub const CFI_EDGE_RELATION: &str = "E";

/// Translate a deterministic cubic CFI graph into a finite relational structure.
///
/// The undirected graph is represented as a directed symmetric binary relation:
/// every canonical edge `{u, v}` contributes both `(u, v)` and `(v, u)`. No
/// distinguished order is attached by this conversion; ordered experiments must
/// make that choice explicitly in a separate step.
///
/// # Errors
///
/// Propagates validation errors from the generic descriptive-complexity
/// substrate. Construction is fail-closed if the graph cannot be represented by
/// that substrate.
pub fn cubic_cfi_as_relational(graph: &CubicCfiGraph) -> Result<FiniteStructure, DescriptiveError> {
    let edge_symbol = RelationSymbol::new(CFI_EDGE_RELATION, 2)?;
    let vocabulary = Vocabulary::new(vec![edge_symbol.clone()])?;

    let mut tuples = Vec::with_capacity(graph.edge_count().saturating_mul(2));
    for &(left, right) in graph.edges() {
        tuples.push(vec![u64::from(left), u64::from(right)]);
        tuples.push(vec![u64::from(right), u64::from(left)]);
    }
    let edge_relation = RelationInterpretation::new(edge_symbol, tuples)?;
    let domain_size = u64::try_from(graph.vertex_count())
        .map_err(|_| DescriptiveError::DomainNotAddressable(u64::MAX))?;
    FiniteStructure::new(domain_size, vocabulary, vec![edge_relation])
}

#[cfg(test)]
mod tests {
    use crate::{CfiBaseGraph, CfiTwistAssignment, CubicCfiGraph};

    use super::*;

    fn k4(twists: Vec<bool>) -> CubicCfiGraph {
        let base = CfiBaseGraph::new(
            4,
            &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)],
        )
        .unwrap();
        let twists = CfiTwistAssignment::new(&base, twists).unwrap();
        CubicCfiGraph::new(&base, &twists).unwrap()
    }

    #[test]
    fn bridge_preserves_domain_and_symmetric_edges() {
        let graph = k4(vec![false; 6]);
        let structure = cubic_cfi_as_relational(&graph).unwrap();
        assert_eq!(structure.domain_size(), 40);
        assert_eq!(structure.vocabulary().relations().len(), 1);

        let edge = structure.relation(CFI_EDGE_RELATION).unwrap();
        assert_eq!(edge.tuples().len(), graph.edge_count() * 2);
        for &(left, right) in graph.edges() {
            let forward = [u64::from(left), u64::from(right)];
            let reverse = [u64::from(right), u64::from(left)];
            assert!(edge.contains(&forward));
            assert!(edge.contains(&reverse));
        }
    }

    #[test]
    fn twist_changes_content_address_without_changing_signature() {
        let untwisted = cubic_cfi_as_relational(&k4(vec![false; 6])).unwrap();
        let twisted = cubic_cfi_as_relational(&k4(vec![true, false, false, false, false, false]))
            .unwrap();

        assert_eq!(untwisted.domain_size(), twisted.domain_size());
        assert_eq!(untwisted.vocabulary(), twisted.vocabulary());
        assert_ne!(untwisted.id(), twisted.id());
    }
}
