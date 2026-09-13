import Mathlib.Data.Nat.Basic

namespace ProofLab.Corpus.Minimize

/--
Equality discharged directly by a hypothesis.
PL-1.2 control: dropping `h` yields a strictly weaker claim for re-verification.
Not a novelty claim.
-/
theorem nat_eq_by_hyp (n : Nat) (h : n = 1) : n = 1 :=
  h

end ProofLab.Corpus.Minimize
