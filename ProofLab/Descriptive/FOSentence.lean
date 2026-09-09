import ProofLab.Descriptive.FO

namespace ProofLab.Descriptive

/--
A syntactically closed first-order sentence over a relational vocabulary.

Closedness is stored with the formula so later second-order layers cannot
silently treat an open formula as a sentence.
-/
structure FOSentence (σ : RelationalVocabulary) where
  formula : FOFormula σ
  closed : formula.freeVariables = ∅

namespace FOSentence

/--
Truth of a closed FO sentence on an ordered finite structure.

We quantify over every first-order assignment rather than choosing one
arbitrarily. The carrier is nonempty by `FiniteStructure.size_pos`, so this
quantification cannot make `⊥` vacuously true. Assignment-independence for
closed formulas may later reduce this universal definition to any concrete
assignment without changing the semantic contract here.
-/
def Holds {σ : RelationalVocabulary} (sentence : FOSentence σ)
    (M : OrderedFiniteStructure σ) : Prop :=
  ∀ assignment : Variable → Fin M.base.size, FOFormula.Holds M assignment sentence.formula

/-- The stored matrix of every sentence is syntactically closed. -/
theorem matrix_closed {σ : RelationalVocabulary} (sentence : FOSentence σ) :
    sentence.formula.freeVariables = ∅ :=
  sentence.closed

end FOSentence

namespace Fixtures

/-- Closed tautology used as a positive sentence-semantics control. -/
def foTruth : FOSentence demoVocabulary where
  formula := .top
  closed := rfl

/-- Closed contradiction used to ensure nonempty carriers prevent vacuous truth. -/
def foFalse : FOSentence demoVocabulary where
  formula := .bottom
  closed := rfl

/-- A bound-variable equality sentence exercising syntactic closure. -/
def everyElementEqualsItself : FOSentence demoVocabulary where
  formula := .forall 0 (.equal 0 0)
  closed := by
    simp [FOFormula.freeVariables]

example : foTruth.Holds demoOrdered := by
  intro assignment
  simp [FOSentence.Holds, foTruth, FOFormula.Holds]

example : ¬ foFalse.Holds demoOrdered := by
  intro h
  have hFalse := h (fun _ => demoOrdered.base.defaultElement)
  simpa [FOSentence.Holds, foFalse, FOFormula.Holds] using hFalse

example : everyElementEqualsItself.Holds demoOrdered := by
  intro assignment
  simp [FOSentence.Holds, everyElementEqualsItself, FOFormula.Holds]

end Fixtures

end ProofLab.Descriptive
