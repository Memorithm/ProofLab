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

Owns immutable claim content, deterministic claim identity, claim lifecycle vocabulary and reproducibility metadata.

### `prooflab-lean`

Owns the process boundary `lake env lean <file>`. It does not decide scientific meaning. It returns a normalized kernel-process result which higher layers will seal into a proof artifact with exact digests.

### Lean library

Owns formal definitions, statements and checked proofs. The initial smoke theorem exists only to verify the toolchain path.

## 4. Next substrate increments

The next justified crates are expected to be:

- `prooflab-store`: content-addressed claim/proof artifact store;
- `prooflab-provenance`: dependency DAG and ancestry/impact queries;
- `prooflab-runner`: verification jobs, environment capture and artifact sealing;
- `prooflab-counterexample`: deterministic/sound falsification adapters;
- `prooflab-search`: untrusted proof/conjecture strategy orchestration;
- `prooflab-cli`: user/agent entry point.

They should be added only when their contract is implemented and tested.

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
