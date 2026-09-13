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

Owns immutable claim content, deterministic claim identity, claim lifecycle vocabulary, reproducibility metadata, content-addressed environment locks, drift reports and library `reproduce` checks (integrity + lock binding). Reproduce success is not proof status. The PL-1.1 `falsify` module owns a restricted Nat expression evaluator and content-addressed counterexample / falsification records that may justify `FALSIFIED` only.

### `prooflab-lean`

Owns the process boundary `lake env lean <file>`. It does not decide scientific meaning. It returns a normalized kernel-process result which higher layers will seal into a proof artifact with exact digests. Lock-aware verification and kernel-backed `reproduce` refuse to proceed under environment drift; only `ProofArtifact::new_verified` after Lean acceptance seals proof status. The PL-1.0 `corpus` module wires claims and formal statements to known Lean fixtures and measures accept/reject correctness without novelty claims. The PL-1.1 `false_conjectures` module runs cheap falsification first and refuses proof-search promotion for intentionally false controls; falsification never seals `PROVED`. The PL-1.2 `minimize` module automatically drops assumptions from known proved controls and re-verifies candidates through Lean; rejection is recorded as `RemovalRejected` and never as necessity.

### `prooflab-cli`

Owns the executable UX/agent surface. Subcommands are thin wrappers: core or Lean `reproduce`, PL-1.1 `falsify` battery, PL-1.0 `verify-corpus`, and PL-1.2 `minimize`. Output is JSON reports. The CLI never invents proof status; Lean acceptance via sealed `ProofArtifact` remains the only `PROVED` path.

### Lean library

Owns formal definitions, statements and checked proofs. The initial smoke theorem exists only to verify the toolchain path. The PL-1.0 known-theorem corpus under `ProofLab/Corpus/` exercises additional proof shapes for orchestration measurement only. PL-1.2 minimization fixtures under `ProofLab/Corpus/Minimize/` exercise redundant-assumption removal with kernel re-verification.

## 4. Next substrate increments

The next justified crates are expected to be:

- `prooflab-store`: content-addressed claim/proof artifact store;
- `prooflab-provenance`: dependency DAG and ancestry/impact queries;
- `prooflab-runner`: verification jobs, environment capture and artifact sealing;
- `prooflab-counterexample`: broader deterministic/sound falsification adapters (PL-1.1 Nat subset lives in `prooflab-core::falsify` until a dedicated crate is justified);
- `prooflab-search`: untrusted proof/conjecture strategy orchestration;
- `prooflab-cli`: thin user/agent entry point (`reproduce` / `falsify` / `verify-corpus` / `minimize`) over existing library APIs — present; does not seal `PROVED`.

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
