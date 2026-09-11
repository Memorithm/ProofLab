import ProofLab.Descriptive.PositiveLFPOperator
import ProofLab.Descriptive.RelationalLFPConvergence

namespace ProofLab.Descriptive

namespace PositiveLfpDefinition

/--
Every intrinsically positive LFP definition over an ordered finite structure
reaches a least fixed-point approximant no later than `size ^ arity` rounds.

The result is obtained by translating the definition into its induced
`RelationalLfpOperator`. Monotonicity of that operator was derived from the
positive syntax, so no independent monotonicity assumption is introduced here.
-/
theorem exists_bounded_least_fixed_approximant {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity) :
    ∃ k ≤ M.base.size ^ arity,
      let op := definition.toRelationalOperator M
      op.step (op.approximant k) = op.approximant k ∧
        ∀ candidate : Finset (Tuple M.base.size arity), op.step candidate = candidate →
          op.approximant k ⊆ candidate := by
  simpa using
    RelationalLfpOperator.exists_bounded_least_fixed_approximant
      (definition.toRelationalOperator M)

/--
The same bound yields an explicit stabilization stage for the semantic operator
induced by a positive definition.
-/
theorem exists_stable_approximant_le_tuple_card {σ : RelationalVocabulary} {arity : Nat}
    (M : OrderedFiniteStructure σ) (definition : PositiveLfpDefinition σ arity) :
    ∃ k ≤ M.base.size ^ arity,
      (definition.toRelationalOperator M).approximant k =
        (definition.toRelationalOperator M).approximant (k + 1) := by
  simpa using
    RelationalLfpOperator.exists_stable_approximant_le_tuple_card
      (definition.toRelationalOperator M)

end PositiveLfpDefinition

namespace Fixtures

example : ∃ k ≤ demoOrdered.base.size,
    (markedClosureDefinition.toRelationalOperator demoOrdered).approximant k =
      (markedClosureDefinition.toRelationalOperator demoOrdered).approximant (k + 1) := by
  simpa using
    markedClosureDefinition.exists_stable_approximant_le_tuple_card demoOrdered

end Fixtures

end ProofLab.Descriptive
