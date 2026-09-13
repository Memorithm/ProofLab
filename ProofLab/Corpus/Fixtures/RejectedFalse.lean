import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus.Fixtures

/--
Intentional falsehood used only as a rejection control.
This file is deliberately excluded from the ProofLab Lake library target so
`lake build` stays green; Rust/Lean CI invokes it directly via `lake env lean`.
Not a novelty claim.
-/
theorem two_plus_two_equals_five : (2 : Nat) + 2 = 5 := by
  decide

end ProofLab.Corpus.Fixtures
