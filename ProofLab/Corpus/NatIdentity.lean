import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus

/-- Known reflexivity control. Proof shape: `rfl`. Not a novelty claim. -/
theorem nat_identity (n : Nat) : n = n := by
  rfl

end ProofLab.Corpus
