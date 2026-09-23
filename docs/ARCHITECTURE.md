# ProofLab Architecture

## 1. Research boundary

ProofLab is not a numerical benchmark with a theorem-shaped report. It is a formal-discovery bench whose final proof authority is external to proposal mechanisms.

The architecture separates four concerns:

1. **proposal** — observations, symbolic systems, LLMs, search and external solvers may propose claims or proof steps;
2. **falsification** — cheap counterexample routes attempt to reject bad claims before expensive proof search;
3. **formal verification** — the trusted kernel accepts or rejects a concrete formal artifact;
4. **research provenance** — all inputs, environments, failed attempts and accepted artifacts remain addressable.

## 2. Two orthogonal state spaces

Mathematical status:

`OBSERVED -> CONJECTURED -> FORMALIZED -> PROVED`

with `FALSIFIED` as an explicit terminal scientific outcome and `GENERALIZED` creating a new claim rather than mutating an old one.

Reproducibility status:

`L0 < L1 < L2 < L3`

The two dimensions must never be conflated.

## 3. Initial executable layers

### `prooflab-core`

Owns immutable claim content, deterministic claim identity, claim lifecycle vocabulary, reproducibility metadata, content-addressed environment locks, drift reports and library `reproduce` checks (integrity + lock binding). Reproduce success is not proof status. The PL-1.1 `falsify` module owns a restricted Nat expression evaluator and content-addressed counterexample / falsification records that may justify `FALSIFIED` only. The `falsify_evidence` bridge maps those records into typed `Observation` / `EvidenceClaim` without sealing `PROVED` or promoting conjectures. The PL-2.0 `evidence` module owns typed observation → evidence → (explicit human/agent promotion) → conjecture → (explicit human/agent formalization) → obligation → kernel-result plumbing; empirical evidence cannot auto-upgrade to conjectures or seal `ProofArtifact` / `PROVED` without an `AcceptedKernel`. The `bench_adapter` module provides label-preserving Riemann/TDI stub ingest into `Observation` / `EvidenceClaim` only.

### `prooflab-lean`

Owns the process boundary `lake env lean <file>`. It does not decide scientific meaning. It returns a normalized kernel-process result which higher layers will seal into a proof artifact with exact digests. Lock-aware verification and kernel-backed `reproduce` refuse to proceed under environment drift; only `ProofArtifact::new_verified` after Lean acceptance seals proof status. The PL-1.0 `corpus` module wires claims and formal statements to known Lean fixtures and measures accept/reject correctness without novelty claims. The PL-1.1 `false_conjectures` module runs cheap falsification first and refuses proof-search promotion for intentionally false controls; falsification never seals `PROVED`. The PL-1.2 `minimize` module automatically drops assumptions from known proved controls and re-verifies candidates through Lean; rejection is recorded as `RemovalRejected` and never as necessity.

### `prooflab-cli`

Owns the executable UX/agent surface. Subcommands are thin wrappers: core or Lean `reproduce`, PL-1.1 `falsify` battery, PL-1.0 `verify-corpus`, PL-1.2 `minimize`, and read-only PL-2.0 `inspect` (typed evidence objects or Riemann/TDI stub manifests). Output is JSON reports. The CLI never invents proof status; `inspect` never calls `AcceptedKernel::seal_proof_artifact`. Lean acceptance via sealed `ProofArtifact` remains the only `PROVED` path.

### Lean library

Owns formal definitions, statements and checked proofs. The initial smoke theorem exists only to verify the toolchain path. The PL-1.0 known-theorem corpus under `ProofLab/Corpus/` exercises additional proof shapes for orchestration measurement only. PL-1.2 minimization fixtures under `ProofLab/Corpus/Minimize/` exercise redundant-assumption removal with kernel re-verification.

## 4. Next substrate increments

The next justified crates are expected to be:

- `prooflab-store`: content-addressed claim/proof artifact store;
- `prooflab-provenance`: dependency DAG and ancestry/impact queries;
- `prooflab-runner`: verification jobs, environment capture and artifact sealing;
- `prooflab-counterexample`: broader deterministic/sound falsification adapters (PL-1.1 Nat subset lives in `prooflab-core::falsify` until a dedicated crate is justified);
- `prooflab-search`: untrusted proof/conjecture strategy orchestration;
- `prooflab-cli`: thin user/agent entry point (`reproduce` / `falsify` / `verify-corpus` / `minimize` / `inspect`) over existing library APIs — present; does not seal `PROVED`.

Further crates should be added only when their contract is implemented and tested.

## 5. Target artifact

A future `ProofArtifact` should bind at least:

- claim ID;
- formal statement ID;
- proof source digest;
- imported dependency digest;
- ProofLab revision;
- Lean version and Lean release commit;
- mathlib revision;
- normalized invocation;
- kernel result;
- environment digest;
- parent/candidate provenance;
- determinism level.

Only an accepted kernel result matching all bound inputs may authorize `ClaimStatus::Proved`.

PL-2.0 Stage-0/1/2 makes the upstream chain explicit: `Observation` → `EvidenceClaim` → (`PromotionMeta`) → `ConjectureCandidate` → (`FormalizationMeta`) → `ProofObligation` → `KernelResult` → (`AcceptedKernel`) → `ProofArtifact`. Stage-1 requires explicit conjecture promotion; Stage-2 requires explicit human/agent provenance for the conjecture→formal-statement binding. Neither transition confers proof status. Failed, unknown and timeout outcomes are not acceptance.
