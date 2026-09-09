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

## PL-1.x — Controlled theorem proving

### PL-1.0 — Known theorem reproduction

Create a corpus of already-known Lean theorems covering several proof shapes. Measure proof verification and orchestration correctness, not mathematical novelty.

### PL-1.1 — Controlled false conjectures

Inject intentionally false statements with known counterexamples and measure whether cheap falsification routes reject them before proof search.

### PL-1.2 — Assumption minimization

For known proved statements with redundant assumptions, test automatic removal followed by kernel re-verification.

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

## PL-DC — Descriptive complexity

ProofLab also hosts a dedicated finite descriptive-complexity programme whose long-term target is the ordered finite-structure formulation of P versus NP through `FO(LFP)` versus `ESO`.

This programme begins with known-theorem controls, semantic infrastructure, games, Weisfeiler-Leman and CFI calibration before any original separation mechanism is considered. In particular, unordered CFI/FPC lower bounds are treated as calibration results rather than evidence that P differs from NP.

See [`PL-DC-DESCRIPTIVE-COMPLEXITY-PROGRAMME.md`](PL-DC-DESCRIPTIVE-COMPLEXITY-PROGRAMME.md).
