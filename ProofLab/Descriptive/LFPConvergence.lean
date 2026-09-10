import ProofLab.Descriptive.LFPMeta
import Lean.Elab.Tactic.Omega

namespace ProofLab.Descriptive

namespace LfpOperator

/--
Every bottom-up approximation sequence for a monotone unary operator on
`Fin n` stabilizes after at most `n` growth steps.

The proof is purely finite: if no stage `k ≤ n` were stable, every inclusion
between successive approximants would be strict, so cardinality would increase
at every step. Starting from the empty relation, the `(n + 1)`-st approximant
would then contain at least `n + 1` elements, contradicting that it is a finset
of `Fin n`.
-/
theorem exists_stable_approximant_le_card {n : Nat} (op : LfpOperator n) :
    ∃ k ≤ n, op.approximant k = op.approximant (k + 1) := by
  by_contra hstable
  push_neg at hstable
  have hstrict : ∀ k, k ≤ n →
      (op.approximant k).card < (op.approximant (k + 1)).card := by
    intro k hk
    have hsubset := op.approximant_subset_next k
    have hne := hstable k hk
    exact Finset.card_lt_card (lt_of_le_of_ne hsubset hne)
  have hlower : ∀ k, k ≤ n + 1 → k ≤ (op.approximant k).card := by
    intro k hk
    induction k with
    | zero =>
        simp [approximant]
    | succ k ih =>
        have hk_le_n : k ≤ n := by omega
        have hk_bound : k ≤ (op.approximant k).card := ih (by omega)
        have hgrowth := hstrict k hk_le_n
        omega
  have htoo_large : n + 1 ≤ (op.approximant (n + 1)).card :=
    hlower (n + 1) (by omega)
  have hcarrier_bound : (op.approximant (n + 1)).card ≤ n := by
    have hsubset : op.approximant (n + 1) ⊆ (Finset.univ : Finset (Fin n)) :=
      Finset.subset_univ _
    simpa using Finset.card_le_card hsubset
  omega

/--
A monotone unary operator on a finite carrier therefore has a bottom-up least
fixed point reached by an approximant no later than round `n`.

The returned witness is both a fixed point and contained in every other fixed
point. This is a theorem about the explicit finite `LfpOperator` substrate; it
is not yet a formalization of full FO(LFP) syntax or the Immerman-Vardi theorem.
-/
theorem exists_bounded_least_fixed_approximant {n : Nat} (op : LfpOperator n) :
    ∃ k ≤ n,
      op.step (op.approximant k) = op.approximant k ∧
        ∀ candidate : Finset (Fin n), op.step candidate = candidate →
          op.approximant k ⊆ candidate := by
  obtain ⟨k, hk, hstable⟩ := op.exists_stable_approximant_le_card
  exact ⟨k, hk, op.stable_approximant_is_least_fixed k hstable⟩

end LfpOperator

namespace Fixtures

example : ∃ k ≤ 3, (selectAll 3).approximant k = (selectAll 3).approximant (k + 1) := by
  exact LfpOperator.exists_stable_approximant_le_card (selectAll 3)

end Fixtures

end ProofLab.Descriptive
