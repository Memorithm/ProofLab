# PL-2.0 — Typed scientific evidence (Stage-0)

PL-2.0 Stage-0 implements the smallest tested type system for the
observation → conjecture → obligation → kernel → proof chain (PL-C15).

## Control

1. Immutable content-addressed types in `prooflab-core::evidence`:
   `Observation`, `EvidenceClaim`, `ConjectureCandidate`, `ProofObligation`,
   `KernelResult` / `KernelOutcome`, integrating existing `FormalStatement`
   and `ProofArtifact`.
2. Empirical kinds (`Numerical`, `SolverOutput`, …) and evidence strengths
   (`Suggestive` / `Corroborated` / `Strong`) never imply `PROVED`.
3. Only `AcceptedKernel` (from a consistent `KernelOutcome::Accepted`) may
   call `seal_proof_artifact`, which delegates to `ProofArtifact::new_verified`.
4. `Rejected` / `Unknown` / `Timeout` stay distinct from acceptance; serde
   round-trip and forged outcome/status upgrades fail `check_id` /
   `into_accepted`.
5. A tiny `Observation::stub` helper exists for type plumbing; no real
   TDI/Riemann ingest.

## Success criteria

- unit tests cover identity, provenance ID preservation, deny paths for
  empirical sealing, distinct non-accepting kernel outcomes, successful seal
  only via accepted kernel, and serde/status upgrade resistance;
- Lean remains the sole `PROVED` authority;
- no mathematical novelty is claimed.

## Non-claims

This stage defines types and trust-boundary tests only. It does not establish
that empirical regularities become theorems, that the type system reduces
errors in production adapters, or any new mathematical result. `empirical ≠ proof`.
