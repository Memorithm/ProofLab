import ProofLab.Descriptive.FO

namespace ProofLab.Descriptive

/--
First-order matrix formulas for existential second-order logic.

`σ` is the immutable input vocabulary. `τ` is a disjoint vocabulary whose
relations are interpreted by an existentially quantified witness. Keeping the
two vocabularies separate prevents a witness relation from silently replacing
an input relation.
-/
inductive ESOFormula (σ τ : RelationalVocabulary) where
  | top
  | bottom
  | equal (left right : Variable)
  | inputRelation (r : σ.Relation) (args : Fin (σ.arity r) → Variable)
  | witnessRelation (r : τ.Relation) (args : Fin (τ.arity r) → Variable)
  | lessThan (left right : Variable)
  | neg (body : ESOFormula σ τ)
  | and (left right : ESOFormula σ τ)
  | or (left right : ESOFormula σ τ)
  | exists (x : Variable) (body : ESOFormula σ τ)
  | forall (x : Variable) (body : ESOFormula σ τ)

namespace ESOFormula

/-- Free first-order variables in an ESO matrix. -/
def freeVariables {σ τ : RelationalVocabulary} : ESOFormula σ τ → Finset Variable
  | .top | .bottom => ∅
  | .equal left right | .lessThan left right => {left, right}
  | .inputRelation _ args | .witnessRelation _ args => Finset.univ.image args
  | .neg body => freeVariables body
  | .and left right | .or left right => freeVariables left ∪ freeVariables right
  | .exists x body | .forall x body => (freeVariables body).erase x

/-- Interpretation assigned to the existentially quantified relation vocabulary. -/
abbrev WitnessInterpretation (τ : RelationalVocabulary) (size : Nat) :=
  (r : τ.Relation) → Tuple size (τ.arity r) → Prop

/-- Tarskian semantics for an ESO first-order matrix under one witness interpretation. -/
def Holds {σ τ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    (witness : WitnessInterpretation τ M.base.size)
    (assignment : Variable → Fin M.base.size) : ESOFormula σ τ → Prop
  | .top => True
  | .bottom => False
  | .equal left right => assignment left = assignment right
  | .inputRelation r args => M.base.interprets r (fun i => assignment (args i))
  | .witnessRelation r args => witness r (fun i => assignment (args i))
  | .lessThan left right => M.lt (assignment left) (assignment right)
  | .neg body => ¬ Holds M witness assignment body
  | .and left right => Holds M witness assignment left ∧ Holds M witness assignment right
  | .or left right => Holds M witness assignment left ∨ Holds M witness assignment right
  | .exists x body =>
      ∃ value : Fin M.base.size,
        Holds M witness (Function.update assignment x value) body
  | .forall x body =>
      ∀ value : Fin M.base.size,
        Holds M witness (Function.update assignment x value) body

end ESOFormula

/--
An ESO sentence with all first-order variables syntactically closed.

All relations in `τ` are existentially quantified simultaneously. The input
vocabulary `σ` remains fixed by the structure.
-/
structure ESOSentence (σ τ : RelationalVocabulary) where
  matrix : ESOFormula σ τ
  closed : matrix.freeVariables = ∅

namespace ESOSentence

/-- Truth of an ESO sentence on an ordered finite input structure. -/
def Holds {σ τ : RelationalVocabulary} (sentence : ESOSentence σ τ)
    (M : OrderedFiniteStructure σ) : Prop :=
  ∃ witness : ESOFormula.WitnessInterpretation τ M.base.size,
    ∀ assignment : Variable → Fin M.base.size,
      ESOFormula.Holds M witness assignment sentence.matrix

end ESOSentence

namespace Fixtures

/-- One unary existential witness relation used by kernel-checked controls. -/
inductive DemoWitnessRelation
  | selected

/-- Unary witness vocabulary, kept disjoint from `demoVocabulary`. -/
abbrev demoWitnessVocabulary : RelationalVocabulary where
  Relation := DemoWitnessRelation
  arity
    | .selected => 1

/-- `∃R ∀x R(x)`, a satisfiable ESO control. -/
def esoAllSelected : ESOSentence demoVocabulary demoWitnessVocabulary where
  matrix := .forall 0 (.witnessRelation .selected (fun _ => 0))
  closed := by
    simp [ESOFormula.freeVariables]

/-- `∃R ∀x (R(x) ∧ ¬R(x))`, an unsatisfiable ESO control. -/
def esoContradictoryWitness : ESOSentence demoVocabulary demoWitnessVocabulary where
  matrix := .forall 0
    (.and
      (.witnessRelation .selected (fun _ => 0))
      (.neg (.witnessRelation .selected (fun _ => 0))))
  closed := by
    simp [ESOFormula.freeVariables]

example : esoAllSelected.Holds demoOrdered := by
  refine ⟨fun _ _ => True, ?_⟩
  intro assignment
  simp [ESOSentence.Holds, esoAllSelected, ESOFormula.Holds]

example : ¬ esoContradictoryWitness.Holds demoOrdered := by
  intro h
  rcases h with ⟨witness, hWitness⟩
  have hAtDefault := hWitness (fun _ => demoOrdered.base.defaultElement)
  simpa [ESOSentence.Holds, esoContradictoryWitness, ESOFormula.Holds] using hAtDefault

end Fixtures

end ProofLab.Descriptive
