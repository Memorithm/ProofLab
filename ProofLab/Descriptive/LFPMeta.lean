import ProofLab.Descriptive.LFP

namespace ProofLab.Descriptive

namespace LfpOperator

/--
Every bottom-up approximant is contained in every pre-fixed point of the
monotone operator.

This is the core leastness invariant behind the least-fixed-point construction:
if `candidate` satisfies `step candidate ⊆ candidate`, then iteration from `∅`
can never leave `candidate`.
-/
theorem approximant_subset_prefixed {n : Nat} (op : LfpOperator n)
    (candidate : Finset (Fin n)) (hpre : op.step candidate ⊆ candidate) (k : Nat) :
    op.approximant k ⊆ candidate := by
  induction k with
  | zero =>
      simp [approximant]
  | succ k ih =>
      exact (op.monotone ih).trans hpre

/--
If one bottom-up approximant is stable, it is a fixed point of the operator.
-/
theorem stable_approximant_is_fixed {n : Nat} (op : LfpOperator n) (k : Nat)
    (hstable : op.approximant k = op.approximant (k + 1)) :
    op.step (op.approximant k) = op.approximant k := by
  rw [hstable]
  rfl

/--
A stable bottom-up approximant is contained in every fixed point and therefore
has the expected leastness property.

This theorem is conditional on a concrete stabilization witness; a general
finite convergence bound is intentionally proved separately rather than hidden
inside this statement.
-/
theorem stable_approximant_subset_fixed {n : Nat} (op : LfpOperator n) (k : Nat)
    (hstable : op.approximant k = op.approximant (k + 1))
    (candidate : Finset (Fin n)) (hfixed : op.step candidate = candidate) :
    op.approximant k ⊆ candidate := by
  have hpre : op.step candidate ⊆ candidate := by
    rw [hfixed]
  exact op.approximant_subset_prefixed candidate hpre k

/-- Once an approximant is stable, the immediately following approximant is stable too. -/
theorem stable_next {n : Nat} (op : LfpOperator n) (k : Nat)
    (hstable : op.approximant k = op.approximant (k + 1)) :
    op.approximant (k + 1) = op.approximant (k + 2) := by
  change op.step (op.approximant k) = op.step (op.approximant (k + 1))
  exact congrArg op.step hstable

end LfpOperator

namespace Fixtures

example : (selectAll 3).approximant 1 = (selectAll 3).approximant 2 := by
  rfl

example : (selectAll 3).step ((selectAll 3).approximant 1) =
    (selectAll 3).approximant 1 := by
  exact LfpOperator.stable_approximant_is_fixed (selectAll 3) 1 (by rfl)

example (candidate : Finset (Fin 3))
    (hfixed : (selectAll 3).step candidate = candidate) :
    (selectAll 3).approximant 1 ⊆ candidate := by
  exact LfpOperator.stable_approximant_subset_fixed (selectAll 3) 1 (by rfl) candidate hfixed

end Fixtures

end ProofLab.Descriptive
