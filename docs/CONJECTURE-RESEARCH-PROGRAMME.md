# ProofLab Conjecture Research Programme

Status: **research bootstrap; no claim is PROVED by this document**.

ProofLab owns the two conjecture lines below because they concern the boundary between empirical evidence, formal statements and machine-checked proof. The Lean kernel remains the only configured authority able to promote a proposition to `PROVED`.

## PL-C3 — Empirical Regularity to Minimal Lemma

### Conjecture
When a numerical or experimental regularity survives a declared battery of independent perturbations, counterexample searches and representation changes, an automated pipeline can often compress the surviving evidence into a substantially smaller candidate statement and assumption set that is more amenable to formal proof or refutation.

### Null
Automated minimization does not reliably reduce statement/assumption complexity without discarding cases, introducing hidden assumptions or overfitting the observed examples.

### Research pipeline
`EvidenceBundle -> CandidateStatement -> AssumptionSet -> CounterexampleSearch -> Minimize -> FormalStatement -> Lean VerificationJob`.

### Stage 0 requirements
- define evidence input schema without treating evidence as proof;
- define statement/assumption complexity metrics;
- preserve provenance from each candidate back to source experiments;
- require adversarial counterexample generation before minimization can advance;
- compare automated minimization to no-minimization and syntactic-only baselines;
- retain rejected candidates and counterexamples as first-class artifacts.

### Initial source adapters
TDI and RiemannBench may provide candidate regularities and evidence manifests. No source label such as `exact`, `numerical`, `conjecture` or `formal asymptotic` may be silently upgraded on import.

## PL-C15 — Typed Scientific Evidence Compiler

### Conjecture
A typed scientific object model can prevent a broad class of epistemic category errors by making invalid promotions such as `NumericalEvidence -> Proof` unrepresentable without an explicit trusted verification transition.

### Null
The type system adds bookkeeping but does not materially reduce invalid claim transitions once real adapters, solvers and agents are integrated.

### Target object chain
`Conjecture -> ExperimentSpec -> Observation -> EvidenceClaim -> LemmaCandidate -> ProofObligation -> FormalStatement -> KernelResult -> TheoremArtifact`.

### Stage 0 requirements
- each object has immutable identity and provenance;
- each transition declares which authority may create it;
- no constructor accepts empirical evidence as a proof artifact;
- external solvers/LLMs may emit candidates, never `PROVED` status;
- failed, unknown and timeout kernel outcomes remain distinct;
- serialization/replay tests prove that status cannot be upgraded by round-trip or adapter loss;
- Lean acceptance is required for the current `PROVED` transition.

### First implementation slice
Extend the existing claim lifecycle with explicit evidence-level and proof-obligation types plus compile-time/runtime tests demonstrating that empirical and solver evidence cannot construct a `ProofArtifact` without a successful trusted `KernelResult`.

## Relationship to PL-DC
These conjecture programmes are additive and must not weaken or displace the existing descriptive-complexity programme. Reusable finite-structure, counterexample and provenance machinery should be shared where technically appropriate.