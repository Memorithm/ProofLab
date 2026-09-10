import ProofLab.Descriptive.FiniteStructure
import Mathlib.Data.Finset.Card
import Mathlib.Data.Fintype.BigOperators

namespace ProofLab.Descriptive

/--
A monotone least-fixed-point transformer over an explicit finite relation of
arity `arity` on a carrier `Fin size`.

The relation state is represented as a `Finset` of tuples, matching the tuple
shape already used by `FiniteStructure`. Monotonicity is carried as a proof
field so a caller cannot construct the operator without justifying that adding
recursive facts cannot remove facts at the next step.
-/
structure RelationalLfpOperator (size arity : Nat) where
  step : Finset (Tuple size arity) → Finset (Tuple size arity)
  monotone : Monotone step

namespace RelationalLfpOperator

/-- Bottom-up approximation sequence from the empty recursive relation. -/
def approximant {size arity : Nat} (op : RelationalLfpOperator size arity) :
    Nat → Finset (Tuple size arity)
  | 0 => ∅
  | k + 1 => op.step (approximant op k)

/-- Successive relational approximants form an increasing chain. -/
theorem approximant_subset_next {size arity : Nat}
    (op : RelationalLfpOperator size arity) (k : Nat) :
    op.approximant k ⊆ op.approximant (k + 1) := by
  induction k with
  | zero =>
      simp [approximant]
  | succ k ih =>
      simpa [approximant] using op.monotone ih

/-- Membership in a bounded relational approximation is executable. -/
def ContainsAt {size arity : Nat} (op : RelationalLfpOperator size arity)
    (round : Nat) (tuple : Tuple size arity) : Prop :=
  tuple ∈ op.approximant round

/-- The complete `arity`-ary tuple carrier has exactly `size ^ arity` tuples. -/
theorem tuple_univ_card (size arity : Nat) :
    (Finset.univ : Finset (Tuple size arity)).card = size ^ arity := by
  simp [Tuple, Fintype.card_fun]

end RelationalLfpOperator

namespace Fixtures

/-- A relational operator that selects every tuple in one step. -/
def selectAllTuples (size arity : Nat) : RelationalLfpOperator size arity where
  step := fun _ => Finset.univ
  monotone := by
    intro _ _ _
    exact Finset.Subset.rfl

example : (selectAllTuples 2 2).approximant 0 = ∅ := by
  rfl

example : (selectAllTuples 2 2).approximant 1 = Finset.univ := by
  rfl

example : ((selectAllTuples 2 2).approximant 1).card = 4 := by
  simpa using RelationalLfpOperator.tuple_univ_card 2 2

end Fixtures

end ProofLab.Descriptive
