# PL-0 — Trusted Infrastructure

PL-0 validates the research substrate itself. It does not search for novel mathematics.

The controls are:

1. a known-valid Lean theorem must be accepted;
2. a known-invalid Lean theorem must be rejected;
3. an accepted Lean theorem may produce a content-addressed `ProofArtifact` only through the verified kernel boundary;
4. a rejected Lean theorem must produce no `ProofArtifact`;
5. a produced proof artifact must survive integrity-checked insertion and retrieval from the ProofLab store;
6. claim, formal-statement and proof-artifact identity must be deterministic and content-sensitive;
7. proof status must remain independent of determinism metadata;
8. toolchain revisions must be explicit.

## PL-0.1 — proof artifact and provenance substrate

PL-0.1 establishes canonical formal-statement identity, `KernelReceipt`, fail-closed `ProofArtifact` construction, a content-addressed in-memory proof store, and deterministic ancestry/impact queries.

## PL-0.2 — live Lean trust-boundary gate

PL-0.2 adds an ignored Rust integration test that is executed explicitly only inside the pinned Lean CI environment. The gate runs one true theorem and one deliberately false theorem through `LeanKernel::verify_job`.

Success criteria:

- the true theorem exits successfully and yields an integrity-valid `ProofArtifact`;
- that artifact can be stored and read back from `MemoryProofStore`;
- the false theorem is rejected by Lean;
- the rejected run yields no proof artifact;
- the CI injects the exact GitHub revision into the ephemeral proof metadata.

This is an infrastructure control, not evidence of mathematical novelty.
