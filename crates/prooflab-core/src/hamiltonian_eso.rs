//! Sound finite ESO control for directed Hamiltonian cycle.
//!
//! The witness relation is a strict total order on the input carrier. The
//! first-order matrix requires every consecutive pair in that order to be an
//! input edge and additionally requires the maximum-to-minimum edge. This is
//! deliberately different from guessing an arbitrary transitive `Reach`
//! superset, which would not force least transitive closure.
//!
//! This module provides a finite semantic control only. It does not formalize
//! Fagin's theorem and has no direct implication for `P` versus `NP`.

use crate::{EsoSentence, EsoValidationError, FoAtom, FoFormula, RelationSymbol, Variable};

/// Input edge relation expected by the Hamiltonian-cycle control.
pub const HAMILTONIAN_EDGE_RELATION: &str = "E";

/// Existential binary relation used as the Hamiltonian order witness.
pub const HAMILTONIAN_ORDER_WITNESS: &str = "HamiltonianOrder";

fn relation(name: &str, args: Vec<Variable>) -> FoFormula {
    FoFormula::Atom(FoAtom::Relation {
        name: name.to_owned(),
        args,
    })
}

fn equal(left: Variable, right: Variable) -> FoFormula {
    FoFormula::Atom(FoAtom::Equal(left, right))
}

fn not(formula: FoFormula) -> FoFormula {
    FoFormula::Not(Box::new(formula))
}

fn implies(antecedent: FoFormula, consequent: FoFormula) -> FoFormula {
    FoFormula::Or(vec![not(antecedent), consequent])
}

fn exists(variable: Variable, body: FoFormula) -> FoFormula {
    FoFormula::Exists {
        variable,
        body: Box::new(body),
    }
}

fn forall(variable: Variable, body: FoFormula) -> FoFormula {
    FoFormula::ForAll {
        variable,
        body: Box::new(body),
    }
}

fn order(left: Variable, right: Variable) -> FoFormula {
    relation(HAMILTONIAN_ORDER_WITNESS, vec![left, right])
}

fn edge(left: Variable, right: Variable) -> FoFormula {
    relation(HAMILTONIAN_EDGE_RELATION, vec![left, right])
}

/// Build the closed relational ESO sentence used as the directed Hamiltonian
/// cycle control.
///
/// `HamiltonianOrder` is existentially quantified as a binary relation. The
/// matrix enforces a strict total order, requires an input edge on every
/// consecutive order pair, and requires the order maximum to connect back to
/// the order minimum.
///
/// # Errors
///
/// Returns an ESO validation error only if construction of the fixed witness
/// signature or the syntactic closure invariant fails.
pub fn directed_hamiltonian_cycle_eso() -> Result<EsoSentence, EsoValidationError> {
    let x = Variable(0);
    let y = Variable(1);
    let z = Variable(2);

    let irreflexive = forall(x, not(order(x, x)));

    let transitive = forall(
        x,
        forall(
            y,
            forall(
                z,
                implies(FoFormula::And(vec![order(x, y), order(y, z)]), order(x, z)),
            ),
        ),
    );

    let total = forall(
        x,
        forall(
            y,
            FoFormula::Or(vec![equal(x, y), order(x, y), order(y, x)]),
        ),
    );

    let has_between = exists(z, FoFormula::And(vec![order(x, z), order(z, y)]));
    let consecutive = FoFormula::And(vec![order(x, y), not(has_between)]);
    let consecutive_edges = forall(x, forall(y, implies(consecutive, edge(x, y))));

    let has_greater = exists(z, order(x, z));
    let has_smaller = exists(z, order(z, y));
    let max_to_min = forall(
        x,
        forall(
            y,
            implies(
                FoFormula::And(vec![not(has_greater), not(has_smaller)]),
                edge(x, y),
            ),
        ),
    );

    let body = FoFormula::And(vec![
        irreflexive,
        transitive,
        total,
        consecutive_edges,
        max_to_min,
    ]);

    let witness = RelationSymbol::new(HAMILTONIAN_ORDER_WITNESS, 2)
        .map_err(EsoValidationError::WitnessVocabulary)?;
    EsoSentence::new(vec![witness], body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FiniteStructure, RelationInterpretation, Vocabulary, evaluate_eso_unordered};

    fn graph(edges: &[(u64, u64)]) -> FiniteStructure {
        let edge_symbol = RelationSymbol::new(HAMILTONIAN_EDGE_RELATION, 2).unwrap();
        let vocabulary = Vocabulary::new(vec![edge_symbol.clone()]).unwrap();
        let interpretation = RelationInterpretation::new(
            edge_symbol,
            edges
                .iter()
                .map(|&(left, right)| vec![left, right])
                .collect(),
        )
        .unwrap();
        FiniteStructure::new(3, vocabulary, vec![interpretation]).unwrap()
    }

    #[test]
    fn sentence_is_closed_and_uses_one_binary_witness() {
        let sentence = directed_hamiltonian_cycle_eso().unwrap();
        let edge = RelationSymbol::new(HAMILTONIAN_EDGE_RELATION, 2).unwrap();
        let vocabulary = Vocabulary::new(vec![edge]).unwrap();

        assert!(sentence.body().free_variables().is_empty());
        assert_eq!(sentence.witness_count(), 1);
        assert_eq!(sentence.max_witness_arity(), Some(2));
        assert_eq!(sentence.validate(&vocabulary, false), Ok(()));
    }

    #[test]
    fn directed_triangle_satisfies_the_order_witness_control() {
        let triangle = graph(&[(0, 1), (1, 2), (2, 0)]);
        let sentence = directed_hamiltonian_cycle_eso().unwrap();
        let evaluation = evaluate_eso_unordered(&sentence, &triangle).unwrap();

        assert!(evaluation.satisfied());
        let witness = evaluation.witness().unwrap();
        assert_eq!(witness.len(), 1);
        assert_eq!(witness[0].symbol().name(), HAMILTONIAN_ORDER_WITNESS);
    }

    #[test]
    fn directed_path_fails_the_cycle_control() {
        let path = graph(&[(0, 1), (1, 2)]);
        let sentence = directed_hamiltonian_cycle_eso().unwrap();
        let evaluation = evaluate_eso_unordered(&sentence, &path).unwrap();

        assert!(!evaluation.satisfied());
        assert!(evaluation.witness().is_none());
        assert_eq!(evaluation.witness_assignments_tested(), 512);
    }

    #[test]
    fn a_different_hamiltonian_vertex_order_is_accepted() {
        let cycle = graph(&[(0, 2), (2, 1), (1, 0)]);
        let sentence = directed_hamiltonian_cycle_eso().unwrap();

        assert!(
            evaluate_eso_unordered(&sentence, &cycle)
                .unwrap()
                .satisfied()
        );
    }
}
