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
    prooflab-core/       claim identity, lifecycle, reproducibility types
    prooflab-lean/       trusted Lean invocation boundary
  ProofLab/
    Core/                Lean definitions and trusted smoke fixtures
    Corpus/              future formal theorem corpus
    Experiments/         future formal experiment modules
  experiments/
    PL-0/                trust/reproducibility infrastructure
    PL-1/                known-theorem and false-conjecture controls
  adapters/              future SciRust/TDI/Riemann bridges
  docs/
```

Do not create crates merely to fill the diagram. Add responsibilities only when they have executable contracts and tests.

## Trust model

Proof status and reproducibility are separate dimensions.

A run may be `L3` bit-reproducible and still compute a false statement. Conversely, successful Lean verification is a mathematical status; the corresponding environment and artifact still need reproducibility metadata.

The first end-to-end target is:

```text
Claim -> FormalStatement -> VerificationJob -> Lean -> KernelResult -> ProofArtifact
```

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

ProofLab is at **PL-0 bootstrap**. The repository currently establishes the trust boundary and initial executable substrate; it does not claim autonomous theorem discovery or new mathematical results.

## Non-claims

ProofLab does not claim that:

- numerical evidence is proof;
- absence of a found counterexample establishes truth;
- an LLM can certify a theorem;
- an external solver result is automatically kernel-verified;
- the current bootstrap is a novel theorem-proving algorithm;
- any future result generalizes beyond its recorded assumptions and proof artifact.
