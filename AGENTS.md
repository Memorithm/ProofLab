# ProofLab Coding Agent Instructions

## Mission

ProofLab is an experimental research bench for autonomous formal mathematical discovery.

The target research loop is:

`OBSERVATION -> CONJECTURE -> FORMALIZATION -> FALSIFICATION -> PROOF -> MINIMIZATION -> GENERALIZATION`

The project must make it possible to ingest empirical, symbolic, or externally produced mathematical evidence, turn candidate statements into explicit formal claims, search for counterexamples and proofs, and promote a claim to `PROVED` only when the configured trusted proof kernel accepts it under a recorded environment.

ProofLab is a research project. Do not fabricate novelty, theorem status, benchmark superiority, or scientific conclusions.

## Non-negotiable trust model

The trusted authority for formal proof status is the configured formal proof kernel.

LLMs, search procedures, symbolic engines, SAT/SMT solvers, computer algebra systems, numerical experiments, heuristic theorem provers, SciRust reasoning components, and external agents are untrusted proposers or supporting tools unless their result is reconstructed and accepted by the trusted kernel.

Never infer `PROVED` from:

- tests passing;
- numerical agreement;
- exhaustive checking over a finite sample when the theorem is not finite by construction;
- an LLM assertion;
- an external solver saying `sat`, `unsat`, or `proved` without a kernel-checkable certificate;
- deterministic execution;
- a symbolic simplification result;
- benchmark success;
- absence of a discovered counterexample.

The transition to `PROVED` must require a machine-checkable proof artifact accepted by the configured proof kernel.

## Proof status and reproducibility are different dimensions

Keep mathematical status separate from reproducibility status.

Suggested mathematical lifecycle:

- `OBSERVED`
- `CONJECTURED`
- `FALSIFIED`
- `FORMALIZED`
- `PROVED`
- `GENERALIZED`

Suggested reproducibility levels follow the SciRust/SOS vocabulary:

- `L0`: non-deterministic but recorded;
- `L1`: statistically reproducible;
- `L2`: numerically reproducible within a declared certificate/tolerance;
- `L3`: bit-reproducible.

An `L3` computation may still be mathematically wrong. A proof-status transition must never be derived from the determinism level.

## Initial formal backend

Use Lean 4 as the initial trusted proof environment and mathlib as the initial mathematical library.

The bootstrap pins:

- Lean `v4.33.1`;
- mathlib `v4.33.1`.

Do not silently float these versions. Any upgrade must be explicit, tested, documented, and reflected in reproducibility metadata.

The architecture must permit future proof-kernel adapters without weakening the Lean path.

## Core architecture

Keep the project layered.

### Rust substrate

The Rust side owns:

- claim lifecycle and metadata;
- content-addressed artifacts;
- provenance;
- experiment/run manifests;
- proof-search orchestration;
- counterexample-search orchestration;
- kernel invocation;
- verification records;
- reproducibility metadata;
- CLI and machine interfaces.

### Lean layer

The Lean side owns:

- formal definitions;
- formal statements;
- imported mathematical dependencies;
- theorem proofs;
- proof-checking fixtures;
- formal experiment corpora.

### Boundary

The Rust/Lean boundary must be explicit. A kernel run must record at minimum:

- source claim identifier;
- formal statement identifier;
- Lean version;
- mathlib revision;
- ProofLab revision;
- imported module set or dependency digest;
- proof source digest;
- command/invocation contract;
- exit/result status;
- stdout/stderr or normalized diagnostic artifact;
- verification artifact identifier.

## SciRust / SOS reuse policy

ProofLab must not reimplement generic research-infrastructure primitives when an appropriate, validated implementation already exists in SciRust/SOS.

Initial audited SciRust reference revision:

`Memorithm/SciRust@267af914bd4a72af531c3d22afd222dd62bd3a70`

Before implementing a generic substrate capability, inspect the corresponding SciRust/SOS implementation.

Priority reuse targets:

- immutable content-addressed objects;
- canonical deterministic serialization;
- stable object identifiers;
- determinism-level propagation;
- environment and reproducibility metadata;
- content-addressed object storage;
- provenance DAG traversal;
- environment locking and drift detection;
- run ledgers and reproducible workflow concepts;
- benchmark/evidence record conventions.

Do not copy the entire SOS architecture into ProofLab.

ProofLab owns formal-mathematics-specific semantics:

- claims;
- assumptions;
- formal statements;
- proof obligations;
- counterexamples;
- proof artifacts;
- theorem dependencies;
- proof minimization;
- theorem generalization;
- proof-search policy;
- trusted-kernel verification.

When code is derived from SciRust/SOS:

1. record the exact upstream repository and commit;
2. preserve required copyright and license notices;
3. document material modifications;
4. retain or strengthen tests for inherited invariants;
5. avoid unnecessary divergence from upstream semantics;
6. upstream genuinely generic improvements to SciRust when they benefit both projects;
7. keep ProofLab-specific formal semantics in ProofLab.

Prefer selective extraction or narrow adapters over wholesale copying.

## Scientific discipline

Every experiment must distinguish:

- theorem;
- formally verified lemma;
- finite exhaustive result;
- exact computation;
- numerical evidence;
- heuristic evidence;
- conjecture;
- refutation;
- inconclusive result.

Do not upgrade one category into another in documentation or code.

Negative results are first-class results. If a conjecture fails, retain the counterexample and provenance.

Do not tune against a final holdout after observing it. When a research phase declares a frozen confirmatory set, CI and ordinary agent runs must not consume it unless the protocol explicitly authorizes that run.

## Counterexample-first behavior

Before spending substantial compute on proof search, attempt the cheapest sound falsification routes appropriate to the claim:

- normalization/simplification;
- boundary cases;
- small finite models;
- exhaustive enumeration where the domain is genuinely finite;
- property-based search;
- symbolic counterexample construction;
- exact arithmetic checks;
- solver-assisted search when results are independently checkable.

A discovered counterexample must transition the claim to `FALSIFIED` and be stored as a reproducible artifact.

No counterexample found is not evidence of proof.

## Assumption discipline

All theorem assumptions must be explicit objects or explicit formal binders.

After obtaining a proof, attempt assumption minimization when the cost is reasonable:

1. remove one assumption;
2. re-run verification;
3. keep the removal only if the theorem still verifies;
4. record the dependency change.

Do not claim an assumption is necessary merely because the current proof uses it. Necessity requires its own argument or counterexample.

## Generalization discipline

Generalization is a separate research operation from proof.

A generalized statement creates a new claim. It must pass the same falsification and verification pipeline as any other claim.

Never overwrite the narrower proved theorem with an unverified generalization.

## Code quality

Default rules:

- Rust stable unless a crate explicitly documents another requirement;
- `#![forbid(unsafe_code)]` in trusted substrate crates unless a reviewed design document justifies an exception;
- explicit errors instead of silent fallback;
- deterministic iteration where result identity matters;
- caller-supplied seeds for stochastic search;
- no hidden global state affecting scientific results;
- stable serialization for content-addressed objects;
- no timestamps, random IDs, machine paths, or wall-clock data inside normative content hashes unless explicitly modeled;
- tests for malformed inputs and failure paths;
- fail closed on verification ambiguity.

Do not weaken an invariant to make CI pass.

## Expected workspace direction

The intended bootstrap shape is:

```text
ProofLab/
  crates/
    prooflab-core/
    prooflab-store/
    prooflab-provenance/
    prooflab-lean/
    prooflab-runner/
    prooflab-counterexample/
    prooflab-search/
    prooflab-cli/
  ProofLab/
    Core/
    Corpus/
    Experiments/
  experiments/
    PL-0/
    PL-1/
  adapters/
    scirust/
    tdi/
    riemann/
  docs/
```

Do not create all crates merely to satisfy the diagram. Add a crate only when a tested responsibility exists.

## First implementation target

Build the smallest end-to-end trusted path:

`Claim -> FormalStatement -> VerificationJob -> Lean kernel -> KernelResult -> ProofArtifact`

The first milestone is complete only when:

1. a Rust-side claim can point to a Lean formalization;
2. ProofLab invokes the pinned Lean toolchain;
3. a valid theorem produces a stored verification artifact;
4. an invalid theorem fails closed;
5. the environment and source digests are recorded;
6. the result is reproducible from repository state;
7. tests exercise both acceptance and rejection paths.

## Research series

Use explicit series identifiers.

Initial plan:

- `PL-0.x`: infrastructure, trust boundary, reproducibility, provenance;
- `PL-1.x`: known-theorem reproduction and controlled false-conjecture rejection;
- `PL-2.x`: observation-to-conjecture-to-proof experiments using externally produced mathematical phenomena;
- later series must be preregistered before strong scientific claims.

## Git and PR workflow

For substantive work:

1. inspect current repository state;
2. create a focused branch;
3. implement the smallest coherent slice;
4. run all relevant local validation;
5. update documentation when contracts change;
6. open a PR with exact scope and evidence;
7. fix CI failures rather than bypassing them;
8. merge only when required checks are green or when the repository has no applicable CI yet and the change is independently validated;
9. delete or stop using obsolete experimental paths rather than leaving ambiguous duplicates.

Never claim a PR is green, merged, or validated without checking the actual repository state.

## Cross-project improvements

When a ProofLab improvement is genuinely generic and would materially benefit another Memorithm project, identify the correct ownership boundary.

Examples:

- generic numerical algorithms -> SciRust;
- generic algorithm search -> Forge;
- generic operator research -> TDI when scientifically appropriate;
- CUDA/NVIDIA kernels -> NNIS;
- ProofLab keeps formal proof semantics and proof-kernel integration.

Do not duplicate ownership merely for convenience.

## Stop conditions

Stop and report explicitly when:

- proof status cannot be verified;
- a dependency version cannot be pinned;
- a result depends on unavailable proprietary evidence;
- a claimed theorem conflicts with a kernel result;
- a scientific conclusion would exceed the evidence;
- a required invariant would need to be weakened.

In those cases, preserve the artifacts and state the exact blocker. Do not invent a successful result.
