import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus.Minimize

/--
Known reflexivity with a redundant `True` hypothesis.
PL-1.2 control: the unused assumption is a candidate for automatic removal.
Not a novelty claim.
-/
theorem nat_rfl_redundant (n : Nat) (_unused : True) : n = n := by
  rfl

end ProofLab.Corpus.Minimize
