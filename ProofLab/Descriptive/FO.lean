import ProofLab.Descriptive.FiniteStructure
import Mathlib.Data.Finset.Card

namespace ProofLab.Descriptive

/-- Syntactic first-order variable identifier. -/
abbrev Variable := Nat

/--
First-order formulas over a purely relational vocabulary with a distinguished
strict order. The order atom is interpreted only through an
`OrderedFiniteStructure`; it is not smuggled into the base vocabulary.
-/
inductive FOFormula (σ : RelationalVocabulary) where
  | top
  | bottom
  | equal (left right : Variable)
  | relation (r : σ.Relation) (args : Fin (σ.arity r) → Variable)
  | lessThan (left right : Variable)
  | neg (body : FOFormula σ)
  | and (left right : FOFormula σ)
  | or (left right : FOFormula σ)
  | exists (x : Variable) (body : FOFormula σ)
  | forall (x : Variable) (body : FOFormula σ)

namespace FOFormula

/-- Syntactic quantifier rank. This is not a minimization over equivalent formulas. -/
def quantifierRank {σ : RelationalVocabulary} : FOFormula σ → Nat
  | .top | .bottom | .equal _ _ | .relation _ _ | .lessThan _ _ => 0
  | .neg body => quantifierRank body
  | .and left right | .or left right => max (quantifierRank left) (quantifierRank right)
  | .exists _ body | .forall _ body => quantifierRank body + 1

/-- All syntactic variable identifiers occurring in a formula. -/
def usedVars {σ : RelationalVocabulary} : FOFormula σ → Finset Variable
  | .top | .bottom => ∅
  | .equal left right | .lessThan left right => {left, right}
  | .relation _ args => Finset.univ.image args
  | .neg body => usedVars body
  | .and left right | .or left right => usedVars left ∪ usedVars right
  | .exists x body | .forall x body => insert x (usedVars body)

/-- Free syntactic variable identifiers occurring in a formula. -/
def freeVariables {σ : RelationalVocabulary} : FOFormula σ → Finset Variable
  | .top | .bottom => ∅
  | .equal left right | .lessThan left right => {left, right}
  | .relation _ args => Finset.univ.image args
  | .neg body => freeVariables body
  | .and left right | .or left right => freeVariables left ∪ freeVariables right
  | .exists x body | .forall x body => (freeVariables body).erase x

/-- Number of distinct syntactic variable identifiers used by the formula. -/
def variableCount {σ : RelationalVocabulary} (formula : FOFormula σ) : Nat :=
  (usedVars formula).card

/--
Tarskian semantics on a finite ordered structure. Quantifiers range over the
exact finite carrier and the distinguished `<` atom is interpreted by the
structure's explicit rank order.
-/
def Holds {σ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    (assignment : Variable → Fin M.base.size) : FOFormula σ → Prop
  | .top => True
  | .bottom => False
  | .equal left right => assignment left = assignment right
  | .relation r args => M.base.interprets r (fun i => assignment (args i))
  | .lessThan left right => M.lt (assignment left) (assignment right)
  | .neg body => ¬ Holds M assignment body
  | .and left right => Holds M assignment left ∧ Holds M assignment right
  | .or left right => Holds M assignment left ∨ Holds M assignment right
  | .exists x body =>
      ∃ value : Fin M.base.size, Holds M (Function.update assignment x value) body
  | .forall x body =>
      ∀ value : Fin M.base.size, Holds M (Function.update assignment x value) body

/-- Quantification increases syntactic quantifier rank by exactly one. -/
theorem quantifierRank_exists {σ : RelationalVocabulary} (x : Variable) (body : FOFormula σ) :
    quantifierRank (.exists x body) = quantifierRank body + 1 := by
  rfl

/-- A variable bound by an existential quantifier is not free at that binder. -/
theorem exists_variable_not_free {σ : RelationalVocabulary} (x : Variable) (body : FOFormula σ) :
    x ∉ freeVariables (.exists x body) := by
  simp [freeVariables]

/-- A variable bound by a universal quantifier is not free at that binder. -/
theorem forall_variable_not_free {σ : RelationalVocabulary} (x : Variable) (body : FOFormula σ) :
    x ∉ freeVariables (.forall x body) := by
  simp [freeVariables]

end FOFormula

namespace Fixtures

open FOFormula

/-- Unary demo formula asserting that variable `0` is marked. -/
def markedZero : FOFormula demoVocabulary :=
  .relation .marked (fun _ => 0)

/-- Ordered demo formula asserting `0 < 1`. -/
def zeroBeforeOne : FOFormula demoVocabulary :=
  .lessThan 0 1

example : markedZero.quantifierRank = 0 := by
  rfl

example : (FOFormula.exists 0 markedZero).quantifierRank = 1 := by
  rfl

example : markedZero.freeVariables = {0} := by
  simp [markedZero, FOFormula.freeVariables]

example : zeroBeforeOne.variableCount = 2 := by
  simp [zeroBeforeOne, FOFormula.variableCount, FOFormula.usedVars]

end Fixtures

end ProofLab.Descriptive
