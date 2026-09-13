# ProofLab known-theorem corpus (PL-1.0)

This directory holds already-known Lean theorems used to measure verification and
orchestration correctness. It does **not** claim mathematical novelty.

## Acceptance fixtures

| Module | Proof shape | Statement |
| --- | --- | --- |
| `NatIdentity` | `rfl` | `∀ n, n = n` |
| `NatDecide` | `decide` | `2 + 2 = 4` |
| `NatAddZero` | `induction` | `∀ n, 0 + n = n` |
| `NatCases` | `cases` / `exact` | `∀ n, n = 0 ∨ ∃ m, n = succ m` |

These modules are imported by the `ProofLab` Lake library and must build cleanly.

## Rejection fixture

`Fixtures/RejectedFalse.lean` states `2 + 2 = 5` and is **not** part of the Lake
library target. The Rust corpus runner invokes it through the same
`LeanKernel` / `VerificationJob` path and expects Lean rejection with no
`ProofArtifact`.

## Non-claims

Success here means the trusted kernel path accepted or rejected as expected under
a recorded environment. It does not establish new mathematics, search quality, or
discovery capability.

## Assumption minimization (PL-1.2)

| Module | Role |
| --- | --- |
| `Minimize/NatRflRedundant` | proved with redundant `True` binder |
| `Minimize/NatRflDropTrue` | same goal after removing `True` (expect accept) |
| `Minimize/NatEqByHyp` | proved using hypothesis `h : n = 1` |
| `Fixtures/MinimizeDropHypReject` | weakened `∀ n, n = 1` after dropping `h` (expect reject; not a Lake target) |

Kernel rejection of a removal candidate records `RemovalRejected` only. It does
not establish that the dropped assumption was necessary unless an independent
counterexample or argument is supplied.
