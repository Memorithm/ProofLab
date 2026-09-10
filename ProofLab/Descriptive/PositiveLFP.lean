import ProofLab.Descriptive.FO

namespace ProofLab.Descriptive

/--
Positive least-fixed-point body syntax over one distinguished recursive relation
of arity `arity`.

The recursive relation has no negation constructor around it. Arbitrary
first-order formulas, including negated input predicates, remain available via
`firstOrder`; therefore negativity is excluded only for the recursive relation
itself. This makes positivity structural rather than a post-hoc validation
condition.
-/
inductive PositiveLfpBody (σ : RelationalVocabulary) (arity : Nat) where
  | firstOrder (formula : FOFormula σ)
  | recursive (args : Fin arity → Variable)
  | and (left right : PositiveLfpBody σ arity)
  | or (left right : PositiveLfpBody σ arity)
  | existsQ (x : Variable) (body : PositiveLfpBody σ arity)
  | forallQ (x : Variable) (body : PositiveLfpBody σ arity)

namespace PositiveLfpBody

/-- Syntactic first-order quantifier rank of a positive LFP body. -/
def quantifierRank {σ : RelationalVocabulary} {arity : Nat} :
    PositiveLfpBody σ arity → Nat
  | .firstOrder formula => formula.quantifierRank
  | .recursive _ => 0
  | .and left right | .or left right => max (quantifierRank left) (quantifierRank right)
  | .existsQ _ body | .forallQ _ body => quantifierRank body + 1

/-- Free first-order variables, including variables used by recursive atoms. -/
def freeVariables {σ : RelationalVocabulary} {arity : Nat} :
    PositiveLfpBody σ arity → Finset Variable
  | .firstOrder formula => formula.freeVariables
  | .recursive args => Finset.univ.image args
  | .and left right | .or left right => freeVariables left ∪ freeVariables right
  | .existsQ x body | .forallQ x body => (freeVariables body).erase x

/--
Semantics of one positive LFP body for a supplied interpretation of the
recursive relation.

The input structure is explicitly ordered because embedded first-order formulas
may use the distinguished `<` atom. The recursive interpretation is separate
from the structure vocabulary and receives exact finite tuples.
-/
def Holds {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ)
    (assignment : Variable → Fin M.base.size)
    (recursiveRelation : Tuple M.base.size arity → Prop) :
    PositiveLfpBody σ arity → Prop
  | .firstOrder formula => formula.Holds M assignment
  | .recursive args => recursiveRelation (fun i => assignment (args i))
  | .and left right =>
      Holds M assignment recursiveRelation left ∧
        Holds M assignment recursiveRelation right
  | .or left right =>
      Holds M assignment recursiveRelation left ∨
        Holds M assignment recursiveRelation right
  | .existsQ x body =>
      ∃ value : Fin M.base.size,
        Holds M (Function.update assignment x value) recursiveRelation body
  | .forallQ x body =>
      ∀ value : Fin M.base.size,
        Holds M (Function.update assignment x value) recursiveRelation body

/--
Positive LFP bodies are monotone in the interpretation of their distinguished
recursive relation.

This theorem is the formal payoff of making positivity intrinsic to the syntax:
no separate polarity checker is needed on the Lean side for this fragment.
-/
theorem holds_mono_recursive {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ)
    {R S : Tuple M.base.size arity → Prop}
    (hRS : ∀ tuple, R tuple → S tuple)
    (body : PositiveLfpBody σ arity)
    (assignment : Variable → Fin M.base.size) :
    Holds M assignment R body → Holds M assignment S body := by
  induction body generalizing assignment with
  | firstOrder formula =>
      intro h
      exact h
  | recursive args =>
      intro h
      exact hRS _ h
  | and left right ihLeft ihRight =>
      intro h
      exact ⟨ihLeft assignment h.1, ihRight assignment h.2⟩
  | or left right ihLeft ihRight =>
      intro h
      rcases h with h | h
      · exact Or.inl (ihLeft assignment h)
      · exact Or.inr (ihRight assignment h)
  | existsQ x body ih =>
      intro h
      rcases h with ⟨value, hvalue⟩
      exact ⟨value, ih (Function.update assignment x value) hvalue⟩
  | forallQ x body ih =>
      intro h value
      exact ih (Function.update assignment x value) (h value)

end PositiveLfpBody

/--
One positive relation-valued LFP definition with an explicit tuple-parameter
vector.

Tuple parameters are pairwise distinct and are the only variables allowed to
remain free in the body. This mirrors the executable Rust `LfpDefinition`
scope invariant while making both conditions proof fields on the Lean side.
-/
structure PositiveLfpDefinition (σ : RelationalVocabulary) (arity : Nat) where
  parameters : Fin arity → Variable
  parameters_injective : Function.Injective parameters
  body : PositiveLfpBody σ arity
  scoped : PositiveLfpBody.freeVariables body ⊆ Finset.univ.image parameters

namespace PositiveLfpDefinition

/-- The exact set of tuple-parameter variable identifiers. -/
def parameterVariables {σ : RelationalVocabulary} {arity : Nat}
    (definition : PositiveLfpDefinition σ arity) : Finset Variable :=
  Finset.univ.image definition.parameters

/-- Every free variable of a validated definition is a tuple parameter. -/
theorem freeVariables_subset_parameters {σ : RelationalVocabulary} {arity : Nat}
    (definition : PositiveLfpDefinition σ arity) :
    definition.body.freeVariables ⊆ definition.parameterVariables := by
  exact definition.scoped

end PositiveLfpDefinition

namespace Fixtures

open PositiveLfpBody

/-- Unary recursive atom `R(x₀)`. -/
def recursiveZero : PositiveLfpBody demoVocabulary 1 :=
  .recursive (fun _ => 0)

/-- Positive body `marked(x₀) ∨ R(x₀)`. -/
def markedOrRecursiveZero : PositiveLfpBody demoVocabulary 1 :=
  .or (.firstOrder markedZero) recursiveZero

example : recursiveZero.quantifierRank = 0 := by
  rfl

example : markedOrRecursiveZero.quantifierRank = 0 := by
  rfl

example : markedOrRecursiveZero.freeVariables = {0} := by
  simp [markedOrRecursiveZero, recursiveZero, PositiveLfpBody.freeVariables, markedZero,
    FOFormula.freeVariables]

/-- Binding the recursive tuple variable removes it from the free-variable set. -/
example : (.existsQ 0 recursiveZero : PositiveLfpBody demoVocabulary 1).freeVariables = ∅ := by
  simp [recursiveZero, PositiveLfpBody.freeVariables]

example (assignment : Variable → Fin demoOrdered.base.size)
    (R S : Tuple demoOrdered.base.size 1 → Prop)
    (hRS : ∀ tuple, R tuple → S tuple) :
    PositiveLfpBody.Holds demoOrdered assignment R markedOrRecursiveZero →
      PositiveLfpBody.Holds demoOrdered assignment S markedOrRecursiveZero := by
  exact PositiveLfpBody.holds_mono_recursive demoOrdered hRS markedOrRecursiveZero assignment

end Fixtures

end ProofLab.Descriptive
