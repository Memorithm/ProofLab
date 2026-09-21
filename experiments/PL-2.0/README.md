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
5. A tiny `Observation::stub` helper exists for type plumbing.
6. Label-preserving Riemann / TDI stub adapters (`prooflab-core::bench_adapter`)
   ingest fixture manifests into `Observation` / `EvidenceClaim` only;
   labels `exact` / `numerical` / `conjecture` / `formal_asymptotic` are
   never upgraded (never `PROVED`). Fixtures:
   `fixtures/riemann_stub_manifest.json`, `fixtures/tdi_stub_manifest.json`.
   No live TDI/Riemann network ingest.

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

## Lean typed-kernel bridge (follow-on)

`prooflab-lean::LeanKernel::verify_obligation` classifies the Lean process boundary
into a typed `KernelResult` / `KernelOutcome` (`Accepted` / `Rejected` /
`Unknown` / `Timeout`) and seals `ProofArtifact` only through `AcceptedKernel`.

Raw process diagnostics remain available as `LeanProcessResult`. The legacy
`verify_job` path is unchanged for PL-0/PL-1 orchestration; PL-2.0 evidence-chain
sealing goes through the obligation path.


## Riemann stub adapter (follow-on)

`ingest_riemann_stub` maps one fixture entry into typed evidence while
preserving the external epistemic label. Attempted upgrades
(`numerical` → `exact`, any label → proof-like tokens) fail closed.
Empirical / asymptotic / conjecture rows cannot seal `ProofArtifact`.

Non-claims: fixture ingest is not a scientific result and does not
reproduce RiemannBench numerics. `empirical ≠ proof`.

## TDI stub adapter (follow-on)

`ingest_tdi_stub` is the parallel PL-C3 ingest for TDI-style operator /
structural evidence fixtures. Same label vocabulary and deny paths; URI
scheme is `tdi-stub://` so provenance stays distinct from Riemann rows.
Never seals `PROVED`.

Non-claims: fixture ingest is not operator novelty and does not run live
TDI. `empirical ≠ proof`.
