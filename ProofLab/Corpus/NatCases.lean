import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus

/--
Known Nat dichotomy control.
Proof shape: `cases` with `exact`.
Not a novelty claim.
-/
theorem zero_or_succ (n : Nat) : n = 0 ∨ ∃ m, n = Nat.succ m := by
  cases n with
  | zero =>
      left
      rfl
  | succ m =>
      right
      exact ⟨m, rfl⟩

end ProofLab.Corpus
