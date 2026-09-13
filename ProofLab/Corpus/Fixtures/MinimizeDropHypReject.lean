import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus.Minimize

/--
Weakened form after dropping `h : n = 1` from `nat_eq_by_hyp`.
Intentionally unprovable by `rfl`. Used only as a PL-1.2 rejection control and
**not** imported by the Lake library target.
Kernel rejection here does **not** by itself prove that `h` was necessary; it only
records that this removal candidate failed re-verification.
-/
theorem nat_eq_drop_hyp_reject (n : Nat) : n = 1 := by
  rfl

end ProofLab.Corpus.Minimize
