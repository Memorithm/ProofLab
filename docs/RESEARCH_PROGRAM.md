# ProofLab Research Program

## PL-0.x — Trusted infrastructure

Goal: establish a reproducible boundary between untrusted mathematical proposal mechanisms and a trusted proof kernel.

### PL-0.0 — Bootstrap

Deliverables:

- pinned Lean/mathlib environment;
- machine-checkable smoke theorem;
- Rust claim model;
- explicit Lean process boundary;
- agent contract and trust rules;
- SciRust/SOS reuse map.

No scientific novelty claim is permitted at this stage.

### PL-0.1 — Proof artifacts

Implement a content-addressed `ProofArtifact` binding claim, formal statement, proof source, environment and kernel result.

Acceptance requires both a positive and a negative verification fixture.

### PL-0.2 — Provenance and reproduction

Add append-only storage, theorem dependency DAG, environment locking, drift detection and `prooflab reproduce` semantics.

Status:

- append-only content-addressed proof storage and theorem dependency DAG queries are available via `prooflab-store` / `prooflab-fs-store`;
- content-addressed `EnvironmentLock`, fail-closed drift detection and library `reproduce` semantics are implemented in `prooflab-core`, with Lean re-verification wired through `prooflab-lean`;
- thin `prooflab` CLI (`prooflab-cli`) exposes `reproduce` / `falsify` / `verify-corpus` / `minimize` / read-only `inspect` over the library APIs; CLI success never seals `PROVED`.

## PL-1.x — Controlled theorem proving

### PL-1.0 — Known theorem reproduction

Create a corpus of already-known Lean theorems covering several proof shapes. Measure proof verification and orchestration correctness, not mathematical novelty.

Status:

- started: minimal Lean corpus under `ProofLab/Corpus/` with distinct proof shapes (`rfl`, `decide`, `induction`, `cases`) plus an intentional rejection fixture;
- Rust orchestration in `prooflab-lean::corpus` builds claims / formal statements / `VerificationJob`s and runs them through `LeanKernel` with optional `EnvironmentLock` binding;
- acceptance requires positive kernel acceptance with sealed `ProofArtifact` and intentional rejection with no artifact;
- no mathematical novelty claim is permitted at this stage.

### PL-1.1 — Controlled false conjectures

Inject intentionally false statements with known counterexamples and measure whether cheap falsification routes reject them before proof search.

Status:

- started: restricted Nat cheap falsifier in `prooflab-core::falsify` with content-addressed `CounterexampleWitness` / `FalsificationRecord`;
- controlled false-conjecture catalog and proof-search refusal gate in `prooflab-lean::false_conjectures`;
- battery rejects closed and universal false Nat equalities before any Lean invocation;
- falsification records `ClaimStatus::Falsified` only; Lean remains the sole `PROVED` authority;
- PL-2.0 bridge `evidence_from_falsification` maps validated records into typed
  `Observation` / `EvidenceClaim` (`falsify://pl-1.1/…`); evidence stays `Observed`,
  scientific status stays `Falsified`, conjecture promotion and proof sealing are refused;
- no mathematical novelty claim is permitted at this stage.

### PL-1.2 — Assumption minimization

For known proved statements with redundant assumptions, test automatic removal followed by kernel re-verification.

Status:

- started: leave-one-out candidate generation and curated controls in `prooflab-lean::minimize`;
- known proved fixtures under `ProofLab/Corpus/Minimize/` plus a rejection fixture under `Fixtures/`;
- automatic removal is re-verified through `LeanKernel`; outcomes are `Removable` or `RemovalRejected` only;
- necessity is never claimed from rejection alone; Lean remains the sole `PROVED` authority;
- no mathematical novelty claim is permitted at this stage.

### Agent / UX entry (`prooflab-cli`)

After PL-0.2–PL-1.2 library surfaces exist, `crates/prooflab-cli` provides a thin executable that calls those APIs only. It does not introduce a new proof authority, conjecture engine, or observation ingest path.

## PL-2.x — Experimental-to-formal discovery

Only after PL-0/PL-1 are stable, ingest observations from external Memorithm research benches such as TDI or Riemann-specific experiments.

The scientific question is whether a system can transform observed structure into useful formal conjectures and verified theorems while retaining complete provenance.

Every PL-2 claim must clearly distinguish:

- observation;
- conjecture;
- failed conjecture/counterexample;
- formal statement;
- verified theorem;
- attempted but unproved statement.

### PL-2.0 — Typed scientific evidence (Stage-0 + Stage-1 + Stage-2)

Status:

- started: `prooflab-core::evidence` introduces content-addressed `Observation`,
  `EvidenceClaim`, `ConjectureCandidate`, `ProofObligation`, and `KernelResult`
  with distinct `Accepted` / `Rejected` / `Unknown` / `Timeout` outcomes;
- typestate `AcceptedKernel` is the only evidence-layer gate that may seal a
  `ProofArtifact` (via existing `ProofArtifact::new_verified`);
- `prooflab-lean::LeanKernel::verify_obligation` wires the Lean process boundary
  into typed `KernelResult` outcomes and seals artifacts only via `AcceptedKernel`;
  raw process output is retained as `LeanProcessResult`;
- numerical / solver / stub observations and strong evidence cannot construct
  proof artifacts or set `PROVED`; serde round-trip cannot upgrade status or
  kernel outcome identity;
- label-preserving Riemann / TDI stub adapters (`prooflab-core::bench_adapter`)
  emit `Observation` / `EvidenceClaim` only from fixture manifests;
  epistemic labels are never upgraded and never imply `PROVED`;
- revision-pinned offline bench exports (`prooflab-core::bench_export`) bind
  repository, source revision, export id, payload reference and SHA-256 digest
  into deterministic provenance before emitting `Observation` / `EvidenceClaim`;
  optional local payload-byte verification detects digest mismatch without network I/O;
- **Stage-1**: `ConjectureCandidate::from_evidence` requires explicit
  `PromotionMeta` (`PromotionAuthority::{Human,Agent}`, non-empty promoter id
  and rationale). Numerical / stub / solver observations cannot auto-upgrade;
  `refuse_auto_upgrade_from_empirical` documents the deny path. Status stays
  `Conjectured` (never `PROVED`);
- **Stage-2**: `ProofObligation::from_conjecture` requires explicit
  `FormalizationMeta` (`FormalizationAuthority::{Human,Agent}`, non-empty
  formalizer id and rationale), and these fields participate in the
  content-addressed obligation identity. Formalization records provenance only;
  it never implies kernel acceptance or `PROVED`;
- PL-1.1 falsify bridge (`prooflab-core::falsify_evidence`) admits
  `FalsificationRecord` → typed evidence without proof; refuses conjecture
  promotion and `AcceptedKernel` sealing on that path;
- `prooflab-cli inspect` is a read-only UX over typed evidence JSON and stub
  manifests; it reports integrity/status/labels and never seals `PROVED`;
- no live TDI/Riemann network ingest is required: real campaigns may hand off
  revision-pinned export manifests offline; Lean remains the sole `PROVED` authority;
- see [`CONJECTURE-RESEARCH-PROGRAMME.md`](CONJECTURE-RESEARCH-PROGRAMME.md) PL-C15
  and `experiments/PL-2.0/`.

## PL-DC — Descriptive complexity

ProofLab also hosts a dedicated finite descriptive-complexity programme whose long-term target is the ordered finite-structure formulation of P versus NP through `FO(LFP)` versus `ESO`.

This programme begins with known-theorem controls, semantic infrastructure, games, Weisfeiler-Leman and CFI calibration before any original separation mechanism is considered. In particular, unordered CFI/FPC lower bounds are treated as calibration results rather than evidence that P differs from NP.

See [`PL-DC-DESCRIPTIVE-COMPLEXITY-PROGRAMME.md`](PL-DC-DESCRIPTIVE-COMPLEXITY-PROGRAMME.md).
