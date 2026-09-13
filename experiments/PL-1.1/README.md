# PL-1.1 — Controlled false conjectures

PL-1.1 injects intentionally false Nat statements with known finite
counterexamples and measures whether cheap falsification rejects them before
proof-search promotion.

## Control

1. A catalog in `prooflab-lean::false_conjectures` lists closed and universal
   Nat equalities that are intentionally false.
2. `prooflab-core::falsify` evaluates a restricted Nat expression language and
   seals a content-addressed `CounterexampleWitness` / `FalsificationRecord`
   when sides disagree.
3. The orchestration gate
   `FalseConjectureEntry::refuse_proof_search_promotion` requires falsification
   and refuses Lean proof-search promotion. The battery never invokes
   `LeanKernel`.
4. Recorded scientific status is `ClaimStatus::Falsified` only. Falsification
   never authorizes `ClaimStatus::Proved`.

## Success criteria

- every catalogued false conjecture is falsified by its known witness;
- every falsification blocks proof-search promotion;
- witnesses and records are content-addressed and integrity-checkable;
- Lean remains the only PROVED authority;
- no mathematical novelty is claimed.

## Non-claims

Passing the PL-1.1 battery shows that cheap falsification routing works for a
fixed control set. It does not establish search quality, discovery capability,
completeness of the falsifier, or any new theorem.
