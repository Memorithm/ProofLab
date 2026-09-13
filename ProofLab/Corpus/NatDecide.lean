import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus

/-- Known concrete Nat equality control. Proof shape: `decide`. Not a novelty claim. -/
theorem two_plus_two : (2 : Nat) + 2 = 4 := by
  decide

end ProofLab.Corpus
