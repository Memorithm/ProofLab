# PL-2.0 — Typed scientific evidence (Stage-0 + Stage-1 + Stage-2)

PL-2.0 implements the smallest tested type system for the
observation → conjecture → obligation → kernel → proof chain (PL-C15),
including Stage-1 explicit conjecture promotion and Stage-2 explicit formalization provenance.

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

7. Stage-1: `ConjectureCandidate::from_evidence` requires `PromotionMeta`
   (`PromotionAuthority::{Human,Agent}`, non-empty promoter id + rationale).
   Numerical / stub / solver observations cannot auto-upgrade
   (`refuse_auto_upgrade_from_empirical`). Status is always `Conjectured`,
   never `PROVED`.
8. Stage-2: `ProofObligation::from_conjecture` requires `FormalizationMeta`
   (`FormalizationAuthority::{Human,Agent}`, non-empty formalizer id + rationale).
   These fields are content-addressed into the obligation. Formalization remains
   untrusted for proof status; Lean acceptance is still required.

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

## CLI inspect (follow-on)

`prooflab inspect` is a read-only agent/UX surface over typed evidence JSON and
the Riemann/TDI stub fixtures above. Example:

```bash
cargo run -p prooflab-cli -- inspect --kind riemann-stub-manifest \
  --input experiments/PL-2.0/fixtures/riemann_stub_manifest.json
cargo run -p prooflab-cli -- inspect --kind tdi-stub-manifest \
  --input experiments/PL-2.0/fixtures/tdi_stub_manifest.json
```

It reports integrity, epistemic labels, and claim status. It never seals
`PROVED` and never calls `AcceptedKernel::seal_proof_artifact`.

## Falsify → EvidenceClaim bridge (follow-on)

`evidence_from_falsification` connects PL-1.1 `FalsificationRecord` /
`CounterexampleWitness` into the typed chain as `Observation` + `EvidenceClaim`.
Scientific status remains `Falsified` on the ingest wrapper; evidence objects stay
`Observed`. `refuse_conjecture_from_falsification` and
`refuse_falsify_evidence_proof_seal` document the trust boundary. Never seals
`PROVED`. `empirical ≠ proof`.


## Stage-1 conjecture promotion

`ConjectureCandidate::from_evidence` binds explicit `PromotionMeta` into the
content-addressed candidate (`prooflab-conjecture-candidate:v2`). Human or agent
authority is required; empty promoter id / rationale fail closed. Numerical,
stub, and solver observations have no silent auto-upgrade path
(`refuse_auto_upgrade_from_empirical`). Promotion never seals `PROVED`.
`empirical ≠ proof`.


## Stage-2 formalization provenance

`ProofObligation::from_conjecture` now requires `FormalizationMeta`. The
formalizer authority, identifier, and rationale are bound into
`prooflab-proof-obligation:v2`. This makes conjecture→formal-statement translation
provenance replayable and prevents a silent formalization transition. Human and
agent formalizations are both untrusted with respect to proof status; only a
consistent accepting Lean `KernelResult` may seal `PROVED`.


## CLI promote / formalize

`prooflab promote` consumes a content-addressed `Claim` plus one or more
`EvidenceClaim` JSON objects and writes a `ConjectureCandidate` only after
explicit `PromotionMeta`. `prooflab formalize` consumes that candidate and a
`FormalStatement`, requires explicit `FormalizationMeta`, and writes a
`ProofObligation`. Neither command invokes Lean or can seal `PROVED`.


## Revision-pinned real-bench exports

`prooflab-core::bench_export` defines the production-facing offline ingest contract
for TDI/Riemann-style campaigns. A manifest pins its source repository, exact
revision and export id; every entry pins its epistemic label, payload reference and
SHA-256 payload digest. Manifest entries are canonicalized by `entry_id`, duplicate
IDs fail closed, and revision/payload changes alter manifest identity.

`verify_bench_export_payload` checks locally obtained payload bytes against the
exported digest without network access. `ingest_bench_export` then creates only
`Observation` / `EvidenceClaim` with `Observed` status and embeds the manifest
identity in the source provenance. This is evidence ingest, not proof verification.


## CLI revision-pinned bench ingest

`prooflab ingest-bench-export` consumes a content-addressed `Claim` plus a validated `BenchExportManifest` and entry id, then writes an `Observation` and `EvidenceClaim`. If `--payload` is provided, the local bytes must match the SHA-256 pinned by the manifest before any output object is emitted. Without payload bytes, the report explicitly records `payload_verified=false`; the manifest digest remains provenance, not proof. The command never promotes a conjecture and never seals `PROVED`.
