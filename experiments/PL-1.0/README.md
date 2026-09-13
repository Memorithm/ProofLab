# PL-1.0 — Known theorem reproduction corpus

PL-1.0 starts controlled theorem proving by reproducing already-known Lean
theorems through the trusted ProofLab verification path.

## Control

1. A small Lean corpus under `ProofLab/Corpus/` covers distinct proof shapes
   (`rfl`, `decide`, `induction`, `cases`).
2. Rust orchestration in `prooflab-lean::corpus` builds `Claim`,
   `FormalStatement`, and `VerificationJob` values that point at those sources.
3. Verification runs through `LeanKernel` with optional `EnvironmentLock`
   binding from PL-0.2.
4. Acceptance fixtures must yield integrity-valid `ProofArtifact` values.
5. An intentional false fixture under `ProofLab/Corpus/Fixtures/` must be
   rejected by Lean and must not produce a proof artifact.

## Success criteria

- corpus catalog preparation binds on-disk source digests;
- lock drift fails closed before Lean invocation;
- ignored integration test `corpus_e2e` matches every expected accept/reject outcome (wired into the Lean CI job when workflow permissions allow);
- no mathematical novelty is claimed.

## Non-claims

This experiment measures verification and orchestration correctness only. Passing
the corpus gate does not establish search quality, discovery capability, or any
new theorem.
