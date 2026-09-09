# SciRust / SOS Reuse Plan

## Audited source

Initial source revision:

`Memorithm/SciRust@267af914bd4a72af531c3d22afd222dd62bd3a70`

The audit identified mature generic research-substrate concepts in SciRust's SOS workspace that should not be independently reinvented in ProofLab.

## High-value reuse

### `sos-core`

Relevant concepts:

- immutable objects;
- canonical encoding;
- content-derived IDs;
- determinism levels;
- reproducibility/environment metadata;
- provenance references.

ProofLab has adopted the `L0..L3` determinism vocabulary in `prooflab-core`, with source attribution. The claim lifecycle remains ProofLab-specific.

PL-0.1 also selectively adapts the deterministic canonical-encoding contract from `sos/sos-core/src/canonical.rs`: type tags, fixed-width integers, length-prefixed byte/string values, deterministic ordered sequences and domain-separated hashing. ProofLab uses these rules for formal-statement and proof-artifact identity while retaining its own proof semantics.

### `sos-store`

Relevant concepts:

- append-only content-addressed storage;
- integrity verification on read/write;
- in-memory and filesystem backends;
- deterministic object enumeration;
- explicit garbage collection from named roots.

PL-0.1 implements the first narrow `prooflab-store` backend rather than importing the whole SOS storage system. It adapts the following contracts from `sos/sos-store/src/store.rs`:

- idempotent first-wins writes;
- integrity verification before acceptance and on read;
- deterministic sorted enumeration;
- fail-closed handling of missing provenance dependencies.

The bootstrap store is intentionally in-memory. Filesystem persistence, named roots and garbage collection remain future work and must not be added until a tested ProofLab responsibility requires them.

### `sos-provenance`

Relevant concepts:

- ancestry queries: "what does this proof depend on?";
- descendant/impact queries: "what breaks if this lemma is retracted?";
- deterministic environment capture.

PL-0.1 establishes proof dependencies as explicit `ProofArtifactId` edges and provides deterministic transitive ancestry and descendant-impact queries. This is a theorem/proof DAG, not a copy of the general SOS knowledge graph.

### `sos-repro`

Relevant concepts:

- environment locks;
- explicit drift detection;
- level-aware reproduction contracts;
- refusal to guess when provenance is ambiguous.

Proof artifacts require non-empty ProofLab revision, Lean version, mathlib revision and environment digest before construction succeeds. The stronger environment-lock/drift model remains a later increment.

## What must not be copied wholesale

Do not import the entire SOS research operating system. ProofLab does not need general scientific planning, simulation, publication, curiosity engines, or all SOS workflow abstractions merely to prove theorems.

Do not allow SciRust neural/symbolic theorem-proposal machinery to become a proof authority. Such tools are untrusted proposers until Lean accepts their reconstructed artifact.

## Ownership rule

Generic improvements discovered while adapting SOS should be considered for upstreaming to SciRust when they are not proof-specific.

ProofLab keeps ownership of:

- formal statement semantics;
- proof obligations;
- counterexamples as claim refutations;
- trusted-kernel adapters;
- proof artifacts;
- theorem dependency semantics;
- assumption minimization;
- theorem generalization.

## Licensing

Derived files must retain the applicable SciRust/SOS copyright and license notice and document their upstream commit. ProofLab uses the same PolyForm Noncommercial 1.0.0 licensing family for the bootstrap.
