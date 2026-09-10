import ProofLab.Descriptive.PositiveLFP
import ProofLab.Descriptive.RelationalLFP

namespace ProofLab.Descriptive

namespace PositiveLfpDefinition

/--
Deterministic first-order assignment induced by a candidate tuple for the
explicit parameters of a positive LFP definition.

Parameter variables receive the corresponding tuple component. Variables that
are not parameters receive the structure's canonical default element. The
`free_variables_scoped` field of `PositiveLfpDefinition` guarantees that those
fallback values cannot affect the truth of the definition body.
-/
noncomputable def tupleAssignment {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity)
    (tuple : Tuple M.base.size arity) : Variable → Fin M.base.size := by
  classical
  exact fun x =>
    if h : ∃ i : Fin arity, definition.parameters i = x then
      tuple (Classical.choose h)
    else
      M.base.defaultElement

/-- Every tuple parameter is assigned its matching tuple component. -/
theorem tupleAssignment_parameter {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity)
    (tuple : Tuple M.base.size arity) (i : Fin arity) :
    definition.tupleAssignment M tuple (definition.parameters i) = tuple i := by
  classical
  rw [tupleAssignment]
  split
  · rename_i h
    have hchosen := Classical.choose_spec h
    have hindex : Classical.choose h = i := by
      apply definition.parameters_injective
      exact hchosen
    rw [hindex]
  · rename_i h
    exfalso
    exact h ⟨i, rfl⟩

/--
One semantic application of a positive LFP definition to a current recursive
relation interpretation.

The result contains exactly those tuples whose instantiated body holds when the
recursive atom is interpreted by `current`.
-/
noncomputable def inducedStep {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity)
    (current : Finset (Tuple M.base.size arity)) : Finset (Tuple M.base.size arity) := by
  classical
  exact Finset.univ.filter fun tuple =>
    PositiveLfpBody.Holds M (definition.tupleAssignment M tuple)
      (fun recursiveTuple => recursiveTuple ∈ current) definition.body

/-- Membership in the induced semantic step is exactly body satisfaction. -/
theorem mem_inducedStep_iff {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity)
    (current : Finset (Tuple M.base.size arity)) (tuple : Tuple M.base.size arity) :
    tuple ∈ definition.inducedStep M current ↔
      PositiveLfpBody.Holds M (definition.tupleAssignment M tuple)
        (fun recursiveTuple => recursiveTuple ∈ current) definition.body := by
  classical
  simp [inducedStep]

/--
The semantic step induced by an intrinsically positive definition is monotone
in the current recursive relation.
-/
theorem inducedStep_mono {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity)
    {current larger : Finset (Tuple M.base.size arity)} (hsubset : current ⊆ larger) :
    definition.inducedStep M current ⊆ definition.inducedStep M larger := by
  classical
  intro tuple htuple
  rw [definition.mem_inducedStep_iff M] at htuple ⊢
  exact PositiveLfpBody.holds_mono_recursive M
    (fun recursiveTuple hmem => hsubset hmem)
    definition.body (definition.tupleAssignment M tuple) htuple

/--
Every positive LFP definition induces a monotone relational operator.

The operator's monotonicity is not an extra assumption: it follows from the
structural positivity theorem for `PositiveLfpBody`.
-/
noncomputable def toRelationalOperator {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity) :
    RelationalLfpOperator M.base.size arity where
  step := definition.inducedStep M
  monotone := by
    intro current larger hsubset
    exact definition.inducedStep_mono M hsubset

/--
The first relational approximant evaluates the body with an empty recursive
relation, as required by least-fixed-point semantics.
-/
theorem mem_first_approximant_iff {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity)
    (tuple : Tuple M.base.size arity) :
    tuple ∈ (definition.toRelationalOperator M).approximant 1 ↔
      PositiveLfpBody.Holds M (definition.tupleAssignment M tuple)
        (fun _ => False) definition.body := by
  classical
  simp [RelationalLfpOperator.approximant, toRelationalOperator,
    inducedStep]

end PositiveLfpDefinition

namespace Fixtures

open PositiveLfpBody

/-- Positive unary definition `R(x) := marked(x) ∨ R(x)`. -/
def markedClosureDefinition : PositiveLfpDefinition demoVocabulary 1 where
  parameters := fun _ => 0
  parameters_injective := by
    intro i j _
    exact Subsingleton.elim i j
  body := markedOrRecursiveZero
  free_variables_scoped := by
    simp [markedOrRecursiveZero, recursiveZero, PositiveLfpBody.freeVariables, markedZero,
      FOFormula.freeVariables]

example :
    markedClosureDefinition.tupleAssignment demoOrdered (fun _ => 2) 0 = 2 := by
  exact markedClosureDefinition.tupleAssignment_parameter demoOrdered (fun _ => 2) 0

example :
    (fun _ => (1 : Fin demoOrdered.base.size)) ∈
      (markedClosureDefinition.toRelationalOperator demoOrdered).approximant 1 := by
  rw [markedClosureDefinition.mem_first_approximant_iff demoOrdered]
  simp [markedClosureDefinition, markedOrRecursiveZero, markedZero, recursiveZero,
    PositiveLfpBody.Holds, PositiveLfpDefinition.tupleAssignment,
    FOFormula.Holds, demoOrdered, demoStructure]

end Fixtures

end ProofLab.Descriptive
