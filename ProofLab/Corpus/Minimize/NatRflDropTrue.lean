import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus.Minimize

/--
Same reflexivity goal after removing the redundant `True` hypothesis.
PL-1.2 control: kernel re-verification of an automatic assumption removal.
Not a novelty claim.
-/
theorem nat_rfl_drop_true (n : Nat) : n = n := by
  rfl

end ProofLab.Corpus.Minimize
