# PL-1.2 — Assumption minimization

PL-1.2 takes known proved statements that carry redundant assumptions, forms
automatic leave-one-out removal candidates, and re-verifies each candidate
through the Lean trust boundary.

## Control

1. Catalog entries in `prooflab-lean::minimize` list full binders plus curated
   removal trials bound to on-disk Lean fixtures.
2. `leave_one_out_candidates` automatically enumerates every single-assumption
   removal; catalogued trials must be among those automatic candidates.
3. Full statements and weakened candidates are prepared as content-addressed
   claims / formal statements / `VerificationJob`s and re-checked via
   `LeanKernel`.
4. Outcomes are only `Removable` (Lean accepted after removal) or
   `RemovalRejected` (Lean rejected after removal). Rejection **never** becomes
   a necessity claim without an independent counterexample or argument.
5. Lean remains the sole `PROVED` authority. Minimization reports do not seal
   proof status by themselves.

## Success criteria

- at least one redundant-assumption control where removal is re-accepted;
- at least one control where removal is rejected (without calling it necessary);
- leave-one-out generation covers every catalogued trial;
- content-addressed jobs bind on-disk sources;
- no mathematical novelty is claimed.

## Non-claims

Passing the PL-1.2 battery shows that automatic assumption removal plus kernel
re-verification works for a fixed control set. It does not establish that a
rejected removal was necessary, that the minimizer is complete, or any new
theorem.
