//! Positive-LFP controls for finite directed graphs.
//!
//! These helpers reuse the exact finite LFP evaluator already present in
//! `ProofLab`. They are calibration controls for PL-DC-1 and do not constitute a
//! complexity-class characterization.

use crate::{
    FiniteStructure, FoAtom, LfpAtom, LfpBody, LfpDefinition, LfpEvaluationError,
    LfpValidationError, Variable, evaluate_lfp_unordered,
};

/// Input edge relation used by the directed graph LFP controls.
pub const LFP_GRAPH_EDGE_RELATION: &str = "E";

/// Build the positive binary LFP definition for reflexive-transitive
/// reachability over the input relation `E`.
///
/// The fixed-point relation contains `(x, y)` when `x = y`, when `E(x, y)`, or
/// when there exists `z` with `E(x, z)` and the recursive relation already
/// contains `(z, y)`.
///
/// # Errors
///
/// Returns an LFP validation error if construction of the fixed recursive shape
/// fails. The fixed definition is intended to be validated against a concrete
/// graph vocabulary by the evaluator.
pub fn directed_reachability_lfp() -> Result<LfpDefinition, LfpValidationError> {
    let x = Variable(0);
    let y = Variable(1);
    let z = Variable(2);

    let body = LfpBody::Or(vec![
        LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Equal(x, y))),
        LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Relation {
            name: LFP_GRAPH_EDGE_RELATION.to_owned(),
            args: vec![x, y],
        })),
        LfpBody::Exists {
            variable: z,
            body: Box::new(LfpBody::And(vec![
                LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Relation {
                    name: LFP_GRAPH_EDGE_RELATION.to_owned(),
                    args: vec![x, z],
                })),
                LfpBody::Atom(LfpAtom::Recursive(vec![z, y])),
            ])),
        },
    ]);

    LfpDefinition::new(vec![x, y], body)
}

/// Decide strong connectivity of one explicit finite directed graph by
/// evaluating the positive reachability LFP and checking every ordered pair.
///
/// # Errors
///
/// Returns an LFP evaluation error if the graph vocabulary, recursive scope,
/// explicit state space, or exact fixed-point evaluation is invalid.
pub fn is_strongly_connected_via_lfp(
    structure: &FiniteStructure,
) -> Result<bool, LfpEvaluationError> {
    let definition = directed_reachability_lfp().map_err(LfpEvaluationError::Definition)?;
    let closure = evaluate_lfp_unordered(&definition, structure)?;

    for source in 0..structure.domain_size() {
        for target in 0..structure.domain_size() {
            if !closure.contains(&[source, target]) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RelationInterpretation, RelationSymbol, Vocabulary};

    fn graph(domain_size: u64, edges: &[(u64, u64)]) -> FiniteStructure {
        let edge_symbol = RelationSymbol::new(LFP_GRAPH_EDGE_RELATION, 2).unwrap();
        let vocabulary = Vocabulary::new(vec![edge_symbol.clone()]).unwrap();
        let interpretation = RelationInterpretation::new(
            edge_symbol,
            edges
                .iter()
                .map(|&(left, right)| vec![left, right])
                .collect(),
        )
        .unwrap();
        FiniteStructure::new(domain_size, vocabulary, vec![interpretation]).unwrap()
    }

    #[test]
    fn reachability_builder_is_positive_and_binary() {
        let definition = directed_reachability_lfp().unwrap();
        let edge_symbol = RelationSymbol::new(LFP_GRAPH_EDGE_RELATION, 2).unwrap();
        let vocabulary = Vocabulary::new(vec![edge_symbol]).unwrap();

        assert_eq!(definition.arity(), 2);
        assert_eq!(definition.validate(&vocabulary, false), Ok(()));
    }

    #[test]
    fn directed_cycle_is_strongly_connected() {
        let cycle = graph(3, &[(0, 1), (1, 2), (2, 0)]);
        assert_eq!(is_strongly_connected_via_lfp(&cycle), Ok(true));
    }

    #[test]
    fn directed_path_is_not_strongly_connected() {
        let path = graph(3, &[(0, 1), (1, 2)]);
        assert_eq!(is_strongly_connected_via_lfp(&path), Ok(false));
    }

    #[test]
    fn singleton_graph_is_strongly_connected_by_reflexive_reachability() {
        let singleton = graph(1, &[]);
        assert_eq!(is_strongly_connected_via_lfp(&singleton), Ok(true));
    }

    #[test]
    fn malformed_graph_vocabulary_fails_closed() {
        let wrong_symbol = RelationSymbol::new("R", 2).unwrap();
        let vocabulary = Vocabulary::new(vec![wrong_symbol.clone()]).unwrap();
        let relation = RelationInterpretation::new(wrong_symbol, vec![]).unwrap();
        let structure = FiniteStructure::new(2, vocabulary, vec![relation]).unwrap();

        assert!(matches!(
            is_strongly_connected_via_lfp(&structure),
            Err(LfpEvaluationError::Definition(_))
        ));
    }
}
