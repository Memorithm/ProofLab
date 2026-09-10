//! Exact finite-model evaluation for relational existential second-order syntax.
//!
//! This module gives PL-DC a reference semantics for [`crate::EsoSentence`].
//! Witness relations are enumerated exactly over the explicit finite carrier in
//! canonical witness-name / tuple order. The first satisfying interpretation is
//! returned as a replayable witness.
//!
//! The search is inherently exponential in the total number of possible witness
//! tuples. No heuristic cutoff is silently introduced: the legacy evaluator
//! either completes the exact finite search or fails closed on representational
//! errors, while the bounded entry points reject before allocation/enumeration
//! unless the caller-supplied budget can cover the complete exact witness space.

use core::fmt;

use crate::{
    DescriptiveError, EsoSentence, EsoValidationError, FiniteStructure, FoAssignment,
    FoEvaluationError, OrderedFiniteStructure, RelationInterpretation, RelationSymbol, Vocabulary,
    evaluate_ordered, evaluate_unordered,
};

/// Explicit preflight budget for complete finite ESO witness enumeration.
///
/// The tuple-slot limit bounds `Σ domain^arity` over existential witness
/// relations. The assignment limit bounds the complete Boolean interpretation
/// space `2^slots`. Bounded evaluation is admitted only when both exact bounds
/// fit before any witness tuple universe is allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EsoEvaluationBudget {
    max_witness_tuple_slots: usize,
    max_witness_assignments: u64,
}

impl EsoEvaluationBudget {
    /// Construct an explicit complete-search budget.
    #[must_use]
    pub const fn new(max_witness_tuple_slots: usize, max_witness_assignments: u64) -> Self {
        Self {
            max_witness_tuple_slots,
            max_witness_assignments,
        }
    }

    /// Maximum admitted number of candidate tuples across all witness relations.
    #[must_use]
    pub const fn max_witness_tuple_slots(self) -> usize {
        self.max_witness_tuple_slots
    }

    /// Maximum admitted number of complete witness assignments.
    #[must_use]
    pub const fn max_witness_assignments(self) -> u64 {
        self.max_witness_assignments
    }
}

/// Deterministic outcome of one exact finite ESO evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsoEvaluation {
    satisfied: bool,
    witness_assignments_tested: u64,
    witness: Option<Vec<RelationInterpretation>>,
}

impl EsoEvaluation {
    /// Whether at least one existential relation interpretation satisfied the matrix.
    #[must_use]
    pub const fn satisfied(&self) -> bool {
        self.satisfied
    }

    /// Number of complete witness assignments evaluated before termination.
    #[must_use]
    pub const fn witness_assignments_tested(&self) -> u64 {
        self.witness_assignments_tested
    }

    /// First satisfying witness in canonical witness-symbol order, if one exists.
    #[must_use]
    pub fn witness(&self) -> Option<&[RelationInterpretation]> {
        self.witness.as_deref()
    }
}

/// Evaluate a closed relational ESO sentence on an unordered finite structure.
///
/// # Errors
///
/// Fails closed if sentence validation fails, the explicit witness tuple space
/// cannot be represented, witness construction fails, FO evaluation fails, or
/// deterministic instrumentation overflows.
pub fn evaluate_eso_unordered(
    sentence: &EsoSentence,
    structure: &FiniteStructure,
) -> Result<EsoEvaluation, EsoEvaluationError> {
    evaluate(sentence, structure, None, None)
}

/// Evaluate a closed relational ESO sentence on an unordered structure under
/// an explicit complete-search budget.
///
/// Budget validation occurs before witness universes are allocated and before
/// the first assignment is enumerated. Rejection never returns a partial result.
///
/// # Errors
///
/// Returns [`EsoEvaluationError`] for semantic/representation failures or when
/// the complete witness search cannot be certified to fit the supplied budget.
pub fn evaluate_eso_unordered_bounded(
    sentence: &EsoSentence,
    structure: &FiniteStructure,
    budget: EsoEvaluationBudget,
) -> Result<EsoEvaluation, EsoEvaluationError> {
    evaluate(sentence, structure, None, Some(budget))
}

/// Evaluate a closed relational ESO sentence on a finite structure with its
/// distinguished total order.
///
/// # Errors
///
/// Fails closed under the same conditions as [`evaluate_eso_unordered`], while
/// interpreting every `<` atom through the supplied distinguished order.
pub fn evaluate_eso_ordered(
    sentence: &EsoSentence,
    structure: &OrderedFiniteStructure,
) -> Result<EsoEvaluation, EsoEvaluationError> {
    evaluate(sentence, structure.structure(), Some(structure), None)
}

/// Evaluate an ordered relational ESO sentence under an explicit complete-search budget.
///
/// # Errors
///
/// Returns [`EsoEvaluationError`] for semantic/representation failures or a
/// preflight budget violation.
pub fn evaluate_eso_ordered_bounded(
    sentence: &EsoSentence,
    structure: &OrderedFiniteStructure,
    budget: EsoEvaluationBudget,
) -> Result<EsoEvaluation, EsoEvaluationError> {
    evaluate(
        sentence,
        structure.structure(),
        Some(structure),
        Some(budget),
    )
}

#[derive(Debug, Clone)]
struct WitnessUniverse {
    symbol: RelationSymbol,
    tuples: Vec<Vec<u64>>,
}

fn evaluate(
    sentence: &EsoSentence,
    input: &FiniteStructure,
    order: Option<&OrderedFiniteStructure>,
    budget: Option<EsoEvaluationBudget>,
) -> Result<EsoEvaluation, EsoEvaluationError> {
    sentence
        .validate(input.vocabulary(), order.is_some())
        .map_err(EsoEvaluationError::Validation)?;

    let total_slots = total_witness_tuple_slots(sentence, input.domain_size())?;
    if let Some(budget) = budget {
        validate_budget(total_slots, budget)?;
    }

    let universes = witness_universes(sentence, input.domain_size())?;
    debug_assert_eq!(
        universes
            .iter()
            .map(|universe| universe.tuples.len())
            .sum::<usize>(),
        total_slots
    );

    let mut selection = Vec::new();
    selection
        .try_reserve_exact(total_slots)
        .map_err(|_| EsoEvaluationError::SelectionBufferNotAddressable { slots: total_slots })?;
    selection.resize(total_slots, false);

    let mut witness_assignments_tested = 0u64;
    loop {
        witness_assignments_tested = witness_assignments_tested
            .checked_add(1)
            .ok_or(EsoEvaluationError::AssignmentCounterOverflow)?;

        let witness = materialize_witness(&universes, &selection)?;
        let extended = extend_structure(input, sentence, witness.clone())?;
        let assignment = FoAssignment::default();
        let holds = if let Some(order) = order {
            let extended_ordered = OrderedFiniteStructure::new(extended, order.order().to_vec())
                .map_err(EsoEvaluationError::Structure)?;
            evaluate_ordered(sentence.body(), &extended_ordered, &assignment)
        } else {
            evaluate_unordered(sentence.body(), &extended, &assignment)
        }
        .map_err(EsoEvaluationError::FirstOrder)?;

        if holds {
            return Ok(EsoEvaluation {
                satisfied: true,
                witness_assignments_tested,
                witness: Some(witness),
            });
        }

        if !increment_binary_selection(&mut selection) {
            return Ok(EsoEvaluation {
                satisfied: false,
                witness_assignments_tested,
                witness: None,
            });
        }
    }
}

fn total_witness_tuple_slots(
    sentence: &EsoSentence,
    domain_size: u64,
) -> Result<usize, EsoEvaluationError> {
    sentence
        .witnesses()
        .relations()
        .iter()
        .try_fold(0usize, |total, symbol| {
            total
                .checked_add(checked_witness_tuple_count(symbol, domain_size)?)
                .ok_or(EsoEvaluationError::TotalWitnessTupleSlotsOverflow)
        })
}

fn validate_budget(
    total_slots: usize,
    budget: EsoEvaluationBudget,
) -> Result<(), EsoEvaluationError> {
    if total_slots > budget.max_witness_tuple_slots {
        return Err(EsoEvaluationError::WitnessTupleSlotBudgetExceeded {
            required: total_slots,
            limit: budget.max_witness_tuple_slots,
        });
    }

    if total_slots >= u64::BITS as usize {
        return Err(EsoEvaluationError::WitnessAssignmentSpaceNotAddressable {
            tuple_slots: total_slots,
        });
    }
    let assignments = 1u64 << total_slots;
    if assignments > budget.max_witness_assignments {
        return Err(EsoEvaluationError::WitnessAssignmentBudgetExceeded {
            required: assignments,
            limit: budget.max_witness_assignments,
        });
    }
    Ok(())
}

fn witness_universes(
    sentence: &EsoSentence,
    domain_size: u64,
) -> Result<Vec<WitnessUniverse>, EsoEvaluationError> {
    let mut universes = Vec::new();
    universes
        .try_reserve_exact(sentence.witness_count())
        .map_err(
            |_| EsoEvaluationError::WitnessUniverseBufferNotAddressable {
                witnesses: sentence.witness_count(),
            },
        )?;

    for symbol in sentence.witnesses().relations() {
        universes.push(WitnessUniverse {
            symbol: symbol.clone(),
            tuples: enumerate_relation_tuples(symbol, domain_size)?,
        });
    }
    Ok(universes)
}

fn checked_witness_tuple_count(
    symbol: &RelationSymbol,
    domain_size: u64,
) -> Result<usize, EsoEvaluationError> {
    let domain = usize::try_from(domain_size)
        .map_err(|_| EsoEvaluationError::DomainNotAddressable { domain_size })?;
    let arity = usize::try_from(symbol.arity()).map_err(|_| {
        EsoEvaluationError::WitnessArityNotAddressable {
            relation: symbol.name().to_owned(),
            arity: symbol.arity(),
        }
    })?;

    let mut tuple_count = 1usize;
    for _ in 0..arity {
        tuple_count = tuple_count.checked_mul(domain).ok_or_else(|| {
            EsoEvaluationError::WitnessTupleSpaceOverflow {
                relation: symbol.name().to_owned(),
                domain_size,
                arity: symbol.arity(),
            }
        })?;
    }
    Ok(tuple_count)
}

fn enumerate_relation_tuples(
    symbol: &RelationSymbol,
    domain_size: u64,
) -> Result<Vec<Vec<u64>>, EsoEvaluationError> {
    let arity = usize::try_from(symbol.arity()).map_err(|_| {
        EsoEvaluationError::WitnessArityNotAddressable {
            relation: symbol.name().to_owned(),
            arity: symbol.arity(),
        }
    })?;
    let tuple_count = checked_witness_tuple_count(symbol, domain_size)?;

    let mut tuples = Vec::new();
    tuples.try_reserve_exact(tuple_count).map_err(|_| {
        EsoEvaluationError::WitnessTupleSpaceNotAddressable {
            relation: symbol.name().to_owned(),
            tuples: tuple_count,
        }
    })?;

    if arity == 0 {
        tuples.push(Vec::new());
        return Ok(tuples);
    }

    let mut tuple = Vec::new();
    tuple.try_reserve_exact(arity).map_err(|_| {
        EsoEvaluationError::WitnessTupleBufferNotAddressable {
            relation: symbol.name().to_owned(),
            arity,
        }
    })?;
    tuple.resize(arity, 0);

    loop {
        let mut materialized = Vec::new();
        materialized.try_reserve_exact(arity).map_err(|_| {
            EsoEvaluationError::WitnessTupleBufferNotAddressable {
                relation: symbol.name().to_owned(),
                arity,
            }
        })?;
        materialized.extend_from_slice(&tuple);
        tuples.push(materialized);

        let mut position = arity;
        loop {
            if position == 0 {
                debug_assert_eq!(tuples.len(), tuple_count);
                return Ok(tuples);
            }
            position -= 1;
            tuple[position] += 1;
            if tuple[position] < domain_size {
                break;
            }
            tuple[position] = 0;
        }
    }
}

fn materialize_witness(
    universes: &[WitnessUniverse],
    selection: &[bool],
) -> Result<Vec<RelationInterpretation>, EsoEvaluationError> {
    let mut interpretations = Vec::new();
    interpretations
        .try_reserve_exact(universes.len())
        .map_err(
            |_| EsoEvaluationError::WitnessInterpretationBufferNotAddressable {
                witnesses: universes.len(),
            },
        )?;

    let mut offset = 0usize;
    for universe in universes {
        let end = offset
            .checked_add(universe.tuples.len())
            .ok_or(EsoEvaluationError::TotalWitnessTupleSlotsOverflow)?;
        let bits = selection
            .get(offset..end)
            .ok_or(EsoEvaluationError::SelectionInvariant)?;
        let selected_count = bits.iter().filter(|&&selected| selected).count();
        let mut tuples = Vec::new();
        tuples.try_reserve_exact(selected_count).map_err(|_| {
            EsoEvaluationError::CandidateTupleBufferNotAddressable {
                relation: universe.symbol.name().to_owned(),
                tuples: selected_count,
            }
        })?;
        for (tuple, &selected) in universe.tuples.iter().zip(bits) {
            if selected {
                let mut cloned = Vec::new();
                cloned.try_reserve_exact(tuple.len()).map_err(|_| {
                    EsoEvaluationError::WitnessTupleBufferNotAddressable {
                        relation: universe.symbol.name().to_owned(),
                        arity: tuple.len(),
                    }
                })?;
                cloned.extend_from_slice(tuple);
                tuples.push(cloned);
            }
        }
        interpretations.push(
            RelationInterpretation::new(universe.symbol.clone(), tuples)
                .map_err(EsoEvaluationError::Structure)?,
        );
        offset = end;
    }

    if offset != selection.len() {
        return Err(EsoEvaluationError::SelectionInvariant);
    }
    Ok(interpretations)
}

fn extend_structure(
    input: &FiniteStructure,
    sentence: &EsoSentence,
    witnesses: Vec<RelationInterpretation>,
) -> Result<FiniteStructure, EsoEvaluationError> {
    let mut symbols = input.vocabulary().relations().to_vec();
    symbols.extend(sentence.witnesses().relations().iter().cloned());
    let vocabulary = Vocabulary::new(symbols).map_err(EsoEvaluationError::Structure)?;

    let mut interpretations = input.relations().to_vec();
    interpretations.extend(witnesses);
    FiniteStructure::new(input.domain_size(), vocabulary, interpretations)
        .map_err(EsoEvaluationError::Structure)
}

fn increment_binary_selection(selection: &mut [bool]) -> bool {
    for bit in selection {
        if *bit {
            *bit = false;
        } else {
            *bit = true;
            return true;
        }
    }
    false
}

/// Fail-closed errors for exact finite relational ESO evaluation.
#[derive(Debug)]
pub enum EsoEvaluationError {
    /// The sentence is invalid for the input vocabulary/order mode.
    Validation(EsoValidationError),
    /// The finite carrier cannot be indexed by this platform.
    DomainNotAddressable { domain_size: u64 },
    /// A witness arity cannot be represented by this platform.
    WitnessArityNotAddressable { relation: String, arity: u64 },
    /// `domain_size^arity` overflowed the explicit tuple-space cardinality.
    WitnessTupleSpaceOverflow {
        relation: String,
        domain_size: u64,
        arity: u64,
    },
    /// The explicit tuple universe for one witness relation cannot be allocated.
    WitnessTupleSpaceNotAddressable { relation: String, tuples: usize },
    /// One explicit witness tuple cannot be allocated.
    WitnessTupleBufferNotAddressable { relation: String, arity: usize },
    /// The witness-universe descriptor vector cannot be allocated.
    WitnessUniverseBufferNotAddressable { witnesses: usize },
    /// Summing all witness tuple slots overflowed `usize`.
    TotalWitnessTupleSlotsOverflow,
    /// The bounded evaluator's total witness tuple-slot budget is insufficient.
    WitnessTupleSlotBudgetExceeded { required: usize, limit: usize },
    /// `2^tuple_slots` cannot be represented by the exact assignment counter.
    WitnessAssignmentSpaceNotAddressable { tuple_slots: usize },
    /// The bounded evaluator cannot cover the complete witness assignment space.
    WitnessAssignmentBudgetExceeded { required: u64, limit: u64 },
    /// The Boolean witness-selection vector cannot be allocated.
    SelectionBufferNotAddressable { slots: usize },
    /// The vector of witness interpretations cannot be allocated.
    WitnessInterpretationBufferNotAddressable { witnesses: usize },
    /// Selected tuples for one witness interpretation cannot be allocated.
    CandidateTupleBufferNotAddressable { relation: String, tuples: usize },
    /// Internal witness-selection indexing became inconsistent.
    SelectionInvariant,
    /// Building the extended finite structure failed.
    Structure(DescriptiveError),
    /// Evaluation of the closed FO matrix failed.
    FirstOrder(FoEvaluationError),
    /// The deterministic complete-witness counter overflowed.
    AssignmentCounterOverflow,
}

impl fmt::Display for EsoEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "invalid ESO sentence: {error}"),
            Self::DomainNotAddressable { domain_size } => write!(
                formatter,
                "ESO domain size {domain_size} cannot be indexed on this platform"
            ),
            Self::WitnessArityNotAddressable { relation, arity } => write!(
                formatter,
                "ESO witness relation {relation} has unaddressable arity {arity}"
            ),
            Self::WitnessTupleSpaceOverflow {
                relation,
                domain_size,
                arity,
            } => write!(
                formatter,
                "ESO witness tuple space for {relation} over {domain_size}^{arity} overflows usize"
            ),
            Self::WitnessTupleSpaceNotAddressable { relation, tuples } => write!(
                formatter,
                "ESO witness tuple universe for {relation} with {tuples} tuples cannot be allocated"
            ),
            Self::WitnessTupleBufferNotAddressable { relation, arity } => write!(
                formatter,
                "ESO witness tuple for {relation} with arity {arity} cannot be allocated"
            ),
            Self::WitnessUniverseBufferNotAddressable { witnesses } => write!(
                formatter,
                "ESO witness-universe vector for {witnesses} relations cannot be allocated"
            ),
            Self::TotalWitnessTupleSlotsOverflow => {
                formatter.write_str("ESO total witness tuple-slot count overflows usize")
            }
            Self::WitnessTupleSlotBudgetExceeded { required, limit } => write!(
                formatter,
                "ESO witness universe requires {required} tuple slots, exceeding budget {limit}"
            ),
            Self::WitnessAssignmentSpaceNotAddressable { tuple_slots } => write!(
                formatter,
                "ESO complete witness space 2^{tuple_slots} is not representable by the exact assignment counter"
            ),
            Self::WitnessAssignmentBudgetExceeded { required, limit } => write!(
                formatter,
                "ESO complete witness search requires {required} assignments, exceeding budget {limit}"
            ),
            Self::SelectionBufferNotAddressable { slots } => write!(
                formatter,
                "ESO witness-selection vector with {slots} slots cannot be allocated"
            ),
            Self::WitnessInterpretationBufferNotAddressable { witnesses } => write!(
                formatter,
                "ESO interpretation vector for {witnesses} witnesses cannot be allocated"
            ),
            Self::CandidateTupleBufferNotAddressable { relation, tuples } => write!(
                formatter,
                "ESO candidate relation {relation} with {tuples} tuples cannot be allocated"
            ),
            Self::SelectionInvariant => {
                formatter.write_str("ESO witness-selection indexing invariant failed")
            }
            Self::Structure(error) => write!(formatter, "ESO extended structure failed: {error}"),
            Self::FirstOrder(error) => {
                write!(formatter, "ESO FO matrix evaluation failed: {error}")
            }
            Self::AssignmentCounterOverflow => {
                formatter.write_str("ESO witness-assignment counter overflowed")
            }
        }
    }
}

impl std::error::Error for EsoEvaluationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FoAtom, FoFormula, FoValidationError, Variable};

    fn empty_structure(size: u64) -> FiniteStructure {
        FiniteStructure::new(size, Vocabulary::new(vec![]).unwrap(), vec![]).unwrap()
    }

    fn witness_atom(name: &str, args: Vec<Variable>) -> FoFormula {
        FoFormula::Atom(FoAtom::Relation {
            name: name.to_owned(),
            args,
        })
    }

    #[test]
    fn unary_witness_is_found_in_canonical_binary_order() {
        let x = Variable(0);
        let sentence = EsoSentence::new(
            vec![RelationSymbol::new("H", 1).unwrap()],
            FoFormula::Exists {
                variable: x,
                body: Box::new(witness_atom("H", vec![x])),
            },
        )
        .unwrap();

        let result = evaluate_eso_unordered(&sentence, &empty_structure(2)).unwrap();
        assert!(result.satisfied());
        assert_eq!(result.witness_assignments_tested(), 2);
        let witness = result.witness().unwrap();
        assert_eq!(witness.len(), 1);
        assert_eq!(witness[0].symbol().name(), "H");
        assert_eq!(witness[0].tuples(), &[vec![0]]);
    }

    #[test]
    fn bounded_unary_witness_matches_unbounded_at_exact_space_boundary() {
        let x = Variable(0);
        let sentence = EsoSentence::new(
            vec![RelationSymbol::new("H", 1).unwrap()],
            FoFormula::Exists {
                variable: x,
                body: Box::new(witness_atom("H", vec![x])),
            },
        )
        .unwrap();
        let structure = empty_structure(2);
        let budget = EsoEvaluationBudget::new(2, 4);

        let bounded = evaluate_eso_unordered_bounded(&sentence, &structure, budget).unwrap();
        let unbounded = evaluate_eso_unordered(&sentence, &structure).unwrap();
        assert_eq!(bounded, unbounded);
    }

    #[test]
    fn bounded_evaluation_rejects_tuple_slots_before_allocation() {
        let x = Variable(0);
        let sentence = EsoSentence::new(
            vec![RelationSymbol::new("H", 1).unwrap()],
            FoFormula::Exists {
                variable: x,
                body: Box::new(witness_atom("H", vec![x])),
            },
        )
        .unwrap();

        assert!(matches!(
            evaluate_eso_unordered_bounded(
                &sentence,
                &empty_structure(2),
                EsoEvaluationBudget::new(1, u64::MAX),
            ),
            Err(EsoEvaluationError::WitnessTupleSlotBudgetExceeded {
                required: 2,
                limit: 1
            })
        ));
    }

    #[test]
    fn bounded_evaluation_requires_complete_assignment_budget() {
        let x = Variable(0);
        let sentence = EsoSentence::new(
            vec![RelationSymbol::new("H", 1).unwrap()],
            FoFormula::Exists {
                variable: x,
                body: Box::new(witness_atom("H", vec![x])),
            },
        )
        .unwrap();

        assert!(matches!(
            evaluate_eso_unordered_bounded(
                &sentence,
                &empty_structure(2),
                EsoEvaluationBudget::new(2, 3),
            ),
            Err(EsoEvaluationError::WitnessAssignmentBudgetExceeded {
                required: 4,
                limit: 3
            })
        ));
    }

    #[test]
    fn unsatisfiable_unary_sentence_exhausts_all_four_witnesses() {
        let x = Variable(0);
        let exists_h = FoFormula::Exists {
            variable: x,
            body: Box::new(witness_atom("H", vec![x])),
        };
        let no_h = FoFormula::ForAll {
            variable: x,
            body: Box::new(FoFormula::Not(Box::new(witness_atom("H", vec![x])))),
        };
        let sentence = EsoSentence::new(
            vec![RelationSymbol::new("H", 1).unwrap()],
            FoFormula::And(vec![exists_h, no_h]),
        )
        .unwrap();

        let result = evaluate_eso_unordered(&sentence, &empty_structure(2)).unwrap();
        assert!(!result.satisfied());
        assert_eq!(result.witness_assignments_tested(), 4);
        assert!(result.witness().is_none());
    }

    #[test]
    fn nullary_second_order_witness_has_two_interpretations() {
        let sentence = EsoSentence::new(
            vec![RelationSymbol::new("Q", 0).unwrap()],
            witness_atom("Q", vec![]),
        )
        .unwrap();

        let result = evaluate_eso_unordered(&sentence, &empty_structure(1)).unwrap();
        assert!(result.satisfied());
        assert_eq!(result.witness_assignments_tested(), 2);
        assert_eq!(result.witness().unwrap()[0].tuples(), &[Vec::<u64>::new()]);
    }

    #[test]
    fn witness_free_sentence_is_evaluated_once() {
        let sentence = EsoSentence::new(vec![], FoFormula::True).unwrap();
        let result = evaluate_eso_unordered(&sentence, &empty_structure(2)).unwrap();
        assert!(result.satisfied());
        assert_eq!(result.witness_assignments_tested(), 1);
        assert_eq!(result.witness().unwrap().len(), 0);
    }

    #[test]
    fn ordered_matrix_uses_the_exact_distinguished_order() {
        let x = Variable(0);
        let y = Variable(1);
        let body = FoFormula::Exists {
            variable: x,
            body: Box::new(FoFormula::Exists {
                variable: y,
                body: Box::new(FoFormula::Atom(FoAtom::LessThan(x, y))),
            }),
        };
        let sentence = EsoSentence::new(vec![], body).unwrap();
        let ordered = OrderedFiniteStructure::new(empty_structure(2), vec![1, 0]).unwrap();

        assert!(
            evaluate_eso_ordered(&sentence, &ordered)
                .unwrap()
                .satisfied()
        );
        assert!(matches!(
            evaluate_eso_unordered(&sentence, ordered.structure()),
            Err(EsoEvaluationError::Validation(EsoValidationError::Body(
                FoValidationError::OrderNotAvailable
            )))
        ));
    }

    #[test]
    fn evaluator_is_deterministic() {
        let x = Variable(0);
        let sentence = EsoSentence::new(
            vec![RelationSymbol::new("H", 1).unwrap()],
            FoFormula::Exists {
                variable: x,
                body: Box::new(witness_atom("H", vec![x])),
            },
        )
        .unwrap();
        let structure = empty_structure(3);

        let first = evaluate_eso_unordered(&sentence, &structure).unwrap();
        let second = evaluate_eso_unordered(&sentence, &structure).unwrap();
        assert_eq!(first, second);
    }
}
