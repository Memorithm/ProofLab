import ProofLab.Descriptive.RelationalLFP
import Lean.Elab.Tactic.Omega

namespace ProofLab.Descriptive

namespace RelationalLfpOperator

/--
Every bottom-up approximant is contained in every pre-fixed point of a
relational monotone operator.
-/
theorem approximant_subset_prefixed {size arity : Nat}
    (op : RelationalLfpOperator size arity)
    (candidate : Finset (Tuple size arity))
    (hpre : op.step candidate ⊆ candidate) (k : Nat) :
    op.approximant k ⊆ candidate := by
  induction k with
  | zero =>
      simp [approximant]
  | succ k ih =>
      exact (op.monotone ih).trans hpre

/-- If a relational bottom-up approximant is stable, it is a fixed point. -/
theorem stable_approximant_is_fixed {size arity : Nat}
    (op : RelationalLfpOperator size arity) (k : Nat)
    (hstable : op.approximant k = op.approximant (k + 1)) :
    op.step (op.approximant k) = op.approximant k := by
  simpa [approximant] using hstable.symm

/--
A stable relational approximant is the least fixed point reached by bottom-up
iteration: it is a fixed point and is contained in every other fixed point.
-/
theorem stable_approximant_is_least_fixed {size arity : Nat}
    (op : RelationalLfpOperator size arity) (k : Nat)
    (hstable : op.approximant k = op.approximant (k + 1)) :
    op.step (op.approximant k) = op.approximant k ∧
      ∀ candidate : Finset (Tuple size arity), op.step candidate = candidate →
        op.approximant k ⊆ candidate := by
  constructor
  · exact op.stable_approximant_is_fixed k hstable
  · intro candidate hfixed
    have hpre : op.step candidate ⊆ candidate := by
      rw [hfixed]
    exact op.approximant_subset_prefixed candidate hpre k

/--
Every bottom-up approximation sequence for a monotone `arity`-ary relation on
`Fin size` stabilizes after at most `size ^ arity` strict growth steps.

The proof uses the exact cardinality of the finite tuple carrier. Whenever a
stage is not stable, monotonicity makes the next finset a strict superset and
therefore strictly increases its cardinality. More than `size ^ arity` such
growth steps would exceed the complete tuple carrier.
-/
theorem exists_stable_approximant_le_tuple_card {size arity : Nat}
    (op : RelationalLfpOperator size arity) :
    ∃ k ≤ size ^ arity, op.approximant k = op.approximant (k + 1) := by
  by_contra hstable
  push Not at hstable
  have hstrict : ∀ k, k ≤ size ^ arity →
      (op.approximant k).card < (op.approximant (k + 1)).card := by
    intro k hk
    have hsubset := op.approximant_subset_next k
    have hne := hstable k hk
    exact Finset.card_lt_card (lt_of_le_of_ne hsubset hne)
  have hlower : ∀ k, k ≤ size ^ arity + 1 → k ≤ (op.approximant k).card := by
    intro k hk
    induction k with
    | zero =>
        simp [approximant]
    | succ k ih =>
        have hk_le_bound : k ≤ size ^ arity := by omega
        have hk_bound : k ≤ (op.approximant k).card := ih (by omega)
        have hgrowth := hstrict k hk_le_bound
        omega
  have htoo_large : size ^ arity + 1 ≤
      (op.approximant (size ^ arity + 1)).card :=
    hlower (size ^ arity + 1) (by omega)
  have hcarrier_bound : (op.approximant (size ^ arity + 1)).card ≤ size ^ arity := by
    have hsubset : op.approximant (size ^ arity + 1) ⊆
        (Finset.univ : Finset (Tuple size arity)) :=
      Finset.subset_univ _
    calc
      (op.approximant (size ^ arity + 1)).card ≤
          (Finset.univ : Finset (Tuple size arity)).card :=
        Finset.card_le_card hsubset
      _ = size ^ arity := by
        exact tuple_univ_card size arity
  omega

/--
A monotone relational operator on a finite carrier therefore reaches a least
fixed-point approximant no later than round `size ^ arity`.
-/
theorem exists_bounded_least_fixed_approximant {size arity : Nat}
    (op : RelationalLfpOperator size arity) :
    ∃ k ≤ size ^ arity,
      op.step (op.approximant k) = op.approximant k ∧
        ∀ candidate : Finset (Tuple size arity), op.step candidate = candidate →
          op.approximant k ⊆ candidate := by
  obtain ⟨k, hk, hstable⟩ := op.exists_stable_approximant_le_tuple_card
  exact ⟨k, hk, op.stable_approximant_is_least_fixed k hstable⟩

end RelationalLfpOperator

namespace Fixtures

example : ∃ k ≤ 4,
    (selectAllTuples 2 2).approximant k = (selectAllTuples 2 2).approximant (k + 1) := by
  simpa using
    RelationalLfpOperator.exists_stable_approximant_le_tuple_card (selectAllTuples 2 2)

example : ∃ k ≤ 4,
    (selectAllTuples 2 2).step ((selectAllTuples 2 2).approximant k) =
        (selectAllTuples 2 2).approximant k := by
  obtain ⟨k, hk, hfixed, _⟩ :=
    RelationalLfpOperator.exists_bounded_least_fixed_approximant (selectAllTuples 2 2)
  exact ⟨k, by simpa using hk, hfixed⟩

end Fixtures

end ProofLab.Descriptive
