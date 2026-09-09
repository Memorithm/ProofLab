import ProofLab.Descriptive.LFP
import Mathlib.Data.Finset.Card
import Mathlib.Data.Fintype.Basic

namespace ProofLab.Descriptive

/--
Build the monotone one-step reachability transformer for a finite directed graph
represented by explicit successor sets.

The source is inserted at every round and every vertex with an incoming edge
from the current approximation is added. This is a finite semantic control for
the LFP substrate, not a complexity-class characterization.
-/
def reachabilityOperator {n : Nat} (source : Fin n)
    (successors : Fin n → Finset (Fin n)) : LfpOperator n where
  step := fun current =>
    insert source <| Finset.univ.filter fun y => ∃ x ∈ current, y ∈ successors x
  monotone := by
    intro left right hsubset y hy
    simp only [Finset.mem_insert, Finset.mem_filter, Finset.mem_univ, true_and] at hy ⊢
    rcases hy with hsource | ⟨x, hx, hxy⟩
    · exact Or.inl hsource
    · exact Or.inr ⟨x, hsubset hx, hxy⟩

/-- The designated source appears after the first reachability step. -/
theorem source_mem_reachability_one {n : Nat} (source : Fin n)
    (successors : Fin n → Finset (Fin n)) :
    source ∈ (reachabilityOperator source successors).approximant 1 := by
  simp [reachabilityOperator, LfpOperator.approximant]

namespace Fixtures

/-- Successor map for the directed path `0 → 1 → 2`. -/
def pathSuccessors (x : Fin 3) : Finset (Fin 3) :=
  if x = 0 then {1} else if x = 1 then {2} else ∅

/-- Reachability from `0` on the directed three-vertex path. -/
def pathReachability : LfpOperator 3 :=
  reachabilityOperator 0 pathSuccessors

example : pathReachability.approximant 0 = ∅ := by
  rfl

example : pathReachability.approximant 1 = {0} := by
  native_decide

example : pathReachability.approximant 2 = {0, 1} := by
  native_decide

example : pathReachability.approximant 3 = Finset.univ := by
  native_decide

example : pathReachability.approximant 3 = pathReachability.approximant 4 := by
  native_decide

end Fixtures

end ProofLab.Descriptive
