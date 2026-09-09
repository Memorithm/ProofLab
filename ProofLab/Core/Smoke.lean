import Mathlib

namespace ProofLab

/-- Bootstrap theorem used only to verify that the pinned Lean/mathlib path works. -/
theorem smoke_identity (n : Nat) : n = n := by
  rfl

end ProofLab
