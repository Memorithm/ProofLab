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

ProofLab has already adopted the `L0..L3` determinism vocabulary in `prooflab-core`, with source attribution. The claim lifecycle remains ProofLab-specific.

### `sos-store`

Relevant concepts for the next increment:

- append-only content-addressed storage;
- integrity verification on read/write;
- in-memory and filesystem backends;
- deterministic object enumeration;
- explicit garbage collection from named roots.

Recommendation: selectively transplant/adapt this design into `prooflab-store`, preserving required notices and tests, rather than depending on the full SciRust workspace.

### `sos-provenance`

Relevant concepts:

- ancestry queries: "what does this proof depend on?";
- descendant/impact queries: "what breaks if this lemma is retracted?";
- deterministic environment capture.

Recommendation: adapt into a theorem/claim dependency graph rather than duplicate graph code from scratch.

### `sos-repro`

Relevant concepts:

- environment locks;
- explicit drift detection;
- level-aware reproduction contracts;
- refusal to guess when provenance is ambiguous.

Recommendation: adapt the lock model to pin Lean, mathlib, ProofLab, imported modules and proof-source digests.

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
