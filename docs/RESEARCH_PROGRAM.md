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
- thin `prooflab` CLI (`prooflab-cli`) exposes `reproduce` / `falsify` / `verify-corpus` / `minimize` over the library APIs; CLI success never seals `PROVED`.

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

### PL-2.0 — Typed scientific evidence (Stage-0)

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
- no real TDI/Riemann ingest yet; Lean remains the sole `PROVED` authority;
- see [`CONJECTURE-RESEARCH-PROGRAMME.md`](CONJECTURE-RESEARCH-PROGRAMME.md) PL-C15
  and `experiments/PL-2.0/`.

## PL-DC — Descriptive complexity

ProofLab also hosts a dedicated finite descriptive-complexity programme whose long-term target is the ordered finite-structure formulation of P versus NP through `FO(LFP)` versus `ESO`.

This programme begins with known-theorem controls, semantic infrastructure, games, Weisfeiler-Leman and CFI calibration before any original separation mechanism is considered. In particular, unordered CFI/FPC lower bounds are treated as calibration results rather than evidence that P differs from NP.

See [`PL-DC-DESCRIPTIVE-COMPLEXITY-PROGRAMME.md`](PL-DC-DESCRIPTIVE-COMPLEXITY-PROGRAMME.md).
