import ProofLab.Descriptive.FiniteStructure
import Mathlib.Data.Finset.Lattice

namespace ProofLab.Descriptive

/--
A monotone unary predicate transformer over a finite carrier.

The monotonicity proof is part of the value, so callers cannot construct an
`LfpOperator` without justifying that adding facts to the current relation can
never remove facts from the next approximation.
-/
structure LfpOperator (n : Nat) where
  step : Finset (Fin n) → Finset (Fin n)
  monotone : Monotone step

namespace LfpOperator

/-- Bottom-up approximation sequence used for least-fixed-point semantics. -/
def approximant {n : Nat} (op : LfpOperator n) : Nat → Finset (Fin n)
  | 0 => ∅
  | k + 1 => op.step (approximant op k)

/-- The first approximation is the operator applied to the empty relation. -/
theorem approximant_one {n : Nat} (op : LfpOperator n) :
    op.approximant 1 = op.step ∅ := by
  rfl

/--
If the operator is inflationary at the current approximation, the next
approximation contains the current one.

This deliberately records the local hypothesis instead of silently assuming
all monotone operators are inflationary.
-/
theorem approximant_subset_next {n : Nat} (op : LfpOperator n) (k : Nat)
    (h : op.approximant k ⊆ op.step (op.approximant k)) :
    op.approximant k ⊆ op.approximant (k + 1) := by
  simpa [approximant] using h

/-- Membership in a finite approximation is an executable proposition. -/
def ContainsAt {n : Nat} (op : LfpOperator n) (round : Nat) (x : Fin n) : Prop :=
  x ∈ op.approximant round

end LfpOperator

namespace Fixtures

/-- A monotone operator that immediately selects every element. -/
def selectAll (n : Nat) : LfpOperator n where
  step := fun _ => Finset.univ
  monotone := by
    intro a b hab
    exact Finset.Subset.rfl

example : (selectAll 3).approximant 0 = ∅ := by
  rfl

example : (selectAll 3).approximant 1 = Finset.univ := by
  rfl

example : (0 : Fin 3) ∈ (selectAll 3).approximant 1 := by
  simp [selectAll, LfpOperator.approximant]

end Fixtures

end ProofLab.Descriptive
