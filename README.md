# ProofLab

ProofLab is an experimental research bench for **autonomous formal mathematical discovery**.

Its target loop is:

```text
OBSERVATION -> CONJECTURE -> FORMALIZATION -> FALSIFICATION -> PROOF -> MINIMIZATION -> GENERALIZATION
```

The project is deliberately stricter than an ordinary benchmark harness. Numerical evidence, symbolic manipulation, solver output, search success, or an LLM assertion may motivate a theorem, but none of them establishes proof status. A claim becomes `PROVED` only after the configured trusted proof kernel accepts a machine-checkable proof under a recorded environment.

## Initial trusted environment

The bootstrap uses:

- Lean `v4.33.1` (Lean commit `819816b2e0a3bf405af45ae5c7af2491d8f5bee6`);
- mathlib `v4.33.1`, pinned by commit `0df444a360eaa60ab8c11dca51a86af692955474`.

The initial proof kernel is Lean. The architecture must permit future proof backends without weakening the Lean verification path.

## Architecture

```text
ProofLab/
  crates/
    prooflab-core/       claim identity, lifecycle, reproducibility, env lock / reproduce, cheap falsify, typed evidence + Riemann/TDI stub adapters (PL-2.0)
    prooflab-lean/       trusted Lean invocation boundary (lock-aware verify/reproduce, corpus, false conjectures, minimize)
    prooflab-store/      content-addressed proof artifact store + provenance queries
    prooflab-fs-store/   durable filesystem backend for verified proofs
    prooflab-cli/        thin `prooflab` UX/agent entry (reproduce/falsify/verify-corpus/minimize/inspect)
  ProofLab/
    Core/                Lean definitions and trusted smoke fixtures
    Corpus/              PL-1.0 known-theorem + PL-1.2 minimization fixtures
    Descriptive/         finite-model-theory and descriptive-complexity substrate
  experiments/           preregistered/calibration research artifacts
  adapters/              SciRust/TDI/Riemann bridges (Riemann/TDI stubs → core bench_adapter)
  docs/
```

Do not create crates merely to fill the diagram. Add responsibilities only when they have executable contracts and tests.

## Trust model

Proof status and reproducibility are separate dimensions.

A run may be `L3` bit-reproducible and still compute a false statement. Conversely, successful Lean verification is a mathematical status; the corresponding environment and artifact still need reproducibility metadata.

The end-to-end trust path is:

```text
Observation -> EvidenceClaim -> ConjectureCandidate -> ProofObligation
Claim -> (cheap falsify?) -> FormalStatement -> VerificationJob -> Lean -> KernelResult -> ProofArtifact
```

Cheap falsification may terminate at `FALSIFIED` before proof search. Empirical / solver evidence may motivate conjectures but cannot seal proof status. Only Lean acceptance (via `AcceptedKernel` / `ProofArtifact::new_verified`) seals `PROVED`.

## SciRust / SOS reuse

ProofLab selectively reuses ideas and, where appropriate, code from the SciRust/SOS research substrate instead of rebuilding generic scientific infrastructure.

The initial audited upstream reference is:

`Memorithm/SciRust@267af914bd4a72af531c3d22afd222dd62bd3a70`

Priority reuse areas are canonical content addressing, determinism metadata, reproducibility/environment records, content-addressed storage, provenance DAGs, and run/reproduction contracts. ProofLab retains ownership of formal-claim semantics, Lean integration, counterexamples, proof artifacts, theorem dependency semantics, minimization, and generalization.

See `docs/SCIRUST_REUSE.md` and `AGENTS.md`.

## Validation

Rust:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Lean:

```bash
lake build
```

Combined:

```bash
./scripts/check.sh
```

## Research status

ProofLab is beyond its original PL-0 bootstrap, but it remains a research substrate rather than a source of new complexity-theoretic claims.

Current substrate includes:

- stable claim/formal-statement/proof-artifact and reproducibility types;
- the configured Lean verification boundary and fresh-kernel qualification path;
- PL-1.0 known-theorem corpus orchestration (verification correctness only; no novelty claims);
- PL-1.1 controlled false-conjecture falsification (cheap Nat counterexamples block proof-search promotion; no novelty claims);
- PL-1.2 assumption minimization (automatic leave-one-out removal + Lean re-verification; no necessity-without-argument claims);
- thin `prooflab` CLI wrapping reproduce / falsify / verify-corpus / minimize / read-only inspect library APIs (no new proof authority);
- finite relational and ordered structures;
- FO syntax/evaluation with syntactic quantifier rank and distinct-variable count;
- ESO syntax/evaluation and a finite Hamiltonian-cycle calibration;
- monotone LFP syntax/evaluation with reachability and SCC controls;
- exact finite EF, pebble, and bijective-pebble game oracles;
- oblivious Weisfeiler-Leman comparison;
- deterministic CFI construction and unordered/ordered calibration utilities, including order-sensitivity searches.

These facilities are controls and calibration infrastructure. CFI, WL/pebble indistinguishability, solver output, experiments, numerical evidence, and failed counterexample searches are never promoted to proof status. Only an artifact accepted by the configured formal kernel may be classified as `PROVED`.

The PL-DC programme uses this substrate to study finite ordered FO/ESO/LFP, model-comparison games, CFI calibration, and order robustness. It does **not** claim a proof of `P = NP`, `P != NP`, Immerman-Vardi, Fagin's theorem, or a new lower bound merely from finite experiments.

## Non-claims

ProofLab does not claim that:

- numerical evidence is proof;
- absence of a found counterexample establishes truth;
- an LLM can certify a theorem;
- an external solver result is automatically kernel-verified;
- CFI, WL, EF, pebble-game, or order-sensitivity calibration establishes `P = NP` or `P != NP`;
- the current substrate is itself a novel theorem-proving algorithm;
- reproducing known theorems in the PL-1.0 corpus establishes mathematical novelty;
- rejecting controlled false conjectures in PL-1.1 establishes discovery capability or completeness of falsification;
- any future result generalizes beyond its recorded assumptions and proof artifact.
