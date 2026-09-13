import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus

/--
Known left-identity of Nat addition.
Proof shape: structural `induction` with `rw`.
Not a novelty claim; reproduces a standard Nat fact for orchestration measurement.
`0 + n = n` is not definitional because `Nat.add` recurses on its second argument.
-/
theorem zero_add (n : Nat) : 0 + n = n := by
  induction n with
  | zero => rfl
  | succ n ih =>
      rw [Nat.add_succ, ih]

end ProofLab.Corpus
