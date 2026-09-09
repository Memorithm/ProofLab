import ProofLab.Descriptive.FOSentence

namespace ProofLab.Descriptive

/-- Disjoint union of an input vocabulary and existential witness vocabulary. -/
def sumVocabulary (σ τ : RelationalVocabulary) : RelationalVocabulary where
  Relation := Sum σ.Relation τ.Relation
  arity
    | .inl relation => σ.arity relation
    | .inr relation => τ.arity relation

/--
Extend a finite input structure with an interpretation for every relation in a
second, disjoint witness vocabulary.
-/
def extendFiniteStructure {σ τ : RelationalVocabulary} (M : FiniteStructure σ)
    (witness : (r : τ.Relation) → Tuple M.size (τ.arity r) → Prop) :
    FiniteStructure (sumVocabulary σ τ) where
  size := M.size
  size_pos := M.size_pos
  interprets
    | .inl relation, tuple => M.interprets relation tuple
    | .inr relation, tuple => witness relation tuple

/-- Extend an ordered input structure without changing its distinguished order. -/
def extendOrderedStructure {σ τ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    (witness : (r : τ.Relation) → Tuple M.base.size (τ.arity r) → Prop) :
    OrderedFiniteStructure (sumVocabulary σ τ) where
  base := extendFiniteStructure M.base witness
  rank := M.rank

@[simp]
theorem extendFiniteStructure_input {σ τ : RelationalVocabulary} (M : FiniteStructure σ)
    (witness : (r : τ.Relation) → Tuple M.size (τ.arity r) → Prop)
    (relation : σ.Relation) (tuple : Tuple M.size (σ.arity relation)) :
    (extendFiniteStructure M witness).interprets (.inl relation) tuple ↔
      M.interprets relation tuple := by
  rfl

@[simp]
theorem extendFiniteStructure_witness {σ τ : RelationalVocabulary} (M : FiniteStructure σ)
    (witness : (r : τ.Relation) → Tuple M.size (τ.arity r) → Prop)
    (relation : τ.Relation) (tuple : Tuple M.size (τ.arity relation)) :
    (extendFiniteStructure M witness).interprets (.inr relation) tuple ↔
      witness relation tuple := by
  rfl

@[simp]
theorem extendOrderedStructure_lt {σ τ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    (witness : (r : τ.Relation) → Tuple M.base.size (τ.arity r) → Prop)
    (left right : Fin M.base.size) :
    (extendOrderedStructure M witness).lt left right ↔ M.lt left right := by
  rfl

/--
A relational existential second-order sentence.

`τ` names the second-order relation variables. The first-order matrix is a
closed sentence over the disjoint vocabulary `σ ⊕ τ`; existential semantics
quantify over every possible interpretation of all relations in `τ`.
-/
structure ESOSentence (σ τ : RelationalVocabulary) where
  matrix : FOSentence (sumVocabulary σ τ)

namespace ESOSentence

/-- Standard relational existential second-order semantics on an ordered finite structure. -/
def Holds {σ τ : RelationalVocabulary} (sentence : ESOSentence σ τ)
    (M : OrderedFiniteStructure σ) : Prop :=
  ∃ witness : (r : τ.Relation) → Tuple M.base.size (τ.arity r) → Prop,
    sentence.matrix.Holds (extendOrderedStructure M witness)

end ESOSentence

namespace Fixtures

/-- One unary existential witness relation for controlled ESO fixtures. -/
inductive ChoiceRelation
  | chosen
  deriving DecidableEq, Repr

abbrev choiceVocabulary : RelationalVocabulary where
  Relation := ChoiceRelation
  arity
    | .chosen => 1

/-- Closed matrix asserting that some element belongs to the witness relation. -/
def someChosenMatrix : FOSentence (sumVocabulary demoVocabulary choiceVocabulary) where
  formula := .exists 0 (.relation (.inr .chosen) (fun _ => 0))
  closed := by
    simp [FOFormula.freeVariables]

/-- Controlled ESO sentence `∃Chosen. ∃x. Chosen(x)`. -/
def someChosen : ESOSentence demoVocabulary choiceVocabulary where
  matrix := someChosenMatrix

example : someChosen.Holds demoOrdered := by
  refine ⟨fun _ _ => True, ?_⟩
  intro assignment
  simp [ESOSentence.Holds, FOSentence.Holds, someChosen, someChosenMatrix, FOFormula.Holds]

end Fixtures

end ProofLab.Descriptive
