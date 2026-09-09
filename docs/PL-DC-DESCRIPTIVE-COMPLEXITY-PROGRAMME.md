# PL-DC — Descriptive Complexity Research Programme

## Scope

PL-DC is ProofLab's formal-research line for finite descriptive complexity, with a long-term focus on the ordered finite-structure formulation of the P versus NP problem.

The exact target is:

```text
P = NP  iff  FO(LFP) and ESO have the same expressive power on ordered finite structures.
```

PL-DC does **not** claim to resolve P versus NP. Its purpose is to build a machine-checkable laboratory in which known theorems, lower-bound techniques, counterexamples, candidate invariants, and eventually new conjectures can be represented with explicit assumptions and trusted-kernel verification.

## Why ProofLab owns this line

ProofLab is the authoritative repository because the central objects are formal claims and inexpressibility/proof obligations, not numerical observables.

Other Memorithm projects may provide untrusted evidence or search services:

- **SciRust**: exact graph algorithms, combinatorics, linear algebra, canonical encodings, and reusable deterministic infrastructure;
- **Forge**: candidate-construction and algorithm/invariant search when execution-derived evidence is useful;
- **TDI**: only for carefully scoped empirical studies of structural distinguishability or generated counterexample families; TDI results remain evidence, never proof;
- **ITD Simulator**: no primary role in this programme; its scientific model is unrelated to finite descriptive logic.

Any external result must enter ProofLab as an observation, candidate construction, counterexample, or proof obligation and must not bypass the Lean trust boundary.

## Non-negotiable distinction: ordered versus unordered structures

The P-capturing form of the Immerman-Vardi theorem uses ordered finite structures. Therefore:

- a lower bound established only for unordered graphs does not separate P from NP;
- a symmetry argument that disappears after adding a linear order does not count as progress toward the ordered FO(LFP) versus ESO target;
- every candidate separation mechanism must explicitly state whether and how it survives arbitrary ordered expansions.

This is a stage gate, not a documentation preference.

## Calibration corrections

PL-DC starts by fixing several common oversimplifications before using them as research premises.

### Hamiltonian cycle in ESO

A guessed binary `Reach` relation constrained only to contain edges and be transitively closed is not sufficient to force `Reach` to be the least transitive closure; an arbitrary larger relation can satisfy those constraints.

The initial trusted control should instead use the standard ESO pattern:

1. existentially quantify a binary relation representing a total order of the vertices;
2. verify in first-order logic that the relation is a linear order;
3. require every consecutive pair in that order to be an input edge;
4. require the maximum-to-minimum edge for the cycle version.

This yields a sound finite-structure ESO specification without smuggling in a least-fixed-point operator.

### Pebble games, counting logic, and Weisfeiler-Leman

The implementation must keep distinct:

- ordinary finite-variable pebble games;
- bijective pebble games for finite-variable logic with counting;
- the exact dimension/variable convention used for Weisfeiler-Leman.

The CFI paper states the convention that `k`-variable first-order logic with counting corresponds to `(k-1)`-dimensional Weisfeiler-Leman. Code and documentation must record the chosen indexing convention explicitly rather than silently shifting `k`.

### CFI is a calibration benchmark, not a P != NP proof

Cai-Fuerer-Immerman constructions are central controls for finite-variable/counting lower bounds and for the limitations of fixed-point logic with counting on unordered/colored graph classes.

They are not, by themselves, a separation of ordered FO(LFP) from ESO. PL-DC will use CFI to validate that its game, counting-logic, WL, graph-generation, and lower-bound machinery reproduces known limitations before attempting order-robust questions.

## Research phases

### PL-DC-0 — Formal substrate

Goal: establish executable definitions with no novelty claim.

Deliverables:

- finite relational vocabularies and finite structures;
- ordered finite structures;
- first-order syntax and semantics reused from mathlib where compatible;
- quantifier rank and bounded-variable accounting;
- an explicit ESO syntax/semantics layer for existential relation quantification;
- a monotone LFP syntax/semantics layer with positivity checks or a construction that makes monotonicity explicit;
- canonical finite-structure serialization and provenance identifiers;
- positive and negative semantic fixtures.

Acceptance criterion: small formulas have independently checkable semantics on finite fixtures, and Lean validates the semantic metatheory used by later phases.

### PL-DC-1 — Known theorem controls

Formalize or faithfully encode a control corpus before any new separation conjecture is promoted.

Initial controls:

- reachability / transitive closure via LFP;
- strongly connected directed graphs via LFP;
- Hamiltonian path/cycle via a sound ESO order-witness construction;
- parity non-definability in first-order logic over the pure-equality finite-structure control family using Ehrenfeucht-Fraisse games;
- basic EF-game / quantifier-rank correspondence for the supported fragment or a precisely scoped formal surrogate;
- finite-variable pebble fixtures;
- bijective pebble/counting fixtures;
- WL refinement fixtures with an explicit indexing convention.

Acceptance criterion: ProofLab can reproduce both positive expressibility examples and negative indistinguishability examples without upgrading finite computation to theorem status.

### PL-DC-2 — Executable indistinguishability engine

Build deterministic Rust support for finite model experiments while keeping proof status in Lean.

Candidate components:

- finite-structure generator;
- graph and ordered-graph canonical encodings;
- EF game solver for bounded rounds;
- `k`-pebble game solver for finite instances;
- bijective pebble game solver for finite instances;
- `k`-WL refinement engine;
- witness traces that can be replayed or translated into formal obligations;
- exact cross-checks between game outcomes and formula fragments on small exhaustive universes.

The engine is an untrusted search/oracle layer. A solver result is evidence until reconstructed or checked by the formal layer.

### PL-DC-3 — CFI calibration family

Implement CFI graph generation from an explicitly represented base graph and twist assignment.

Required controls:

- deterministic generation;
- explicit parity of twist assignments;
- isomorphism/non-isomorphism claims stated with exact hypotheses;
- bijective-pebble and WL experiments across increasing dimensions;
- no unsupported asymptotic claim from finite samples;
- formalization of the strongest tractable local correctness lemmas before attempting the full lower-bound theorem.

The purpose is to validate the laboratory against a known hard family.

### PL-DC-4 — Order-robust lower-bound laboratory

This is the first phase directly relevant to the ordered FO(LFP) versus ESO formulation.

Every candidate property or indistinguishability construction must be tested against order expansion.

Research questions include:

- which CFI-style symmetries survive when structures carry an arbitrary linear order;
- whether a candidate game invariant remains meaningful for ordered structures;
- whether order can be factored, randomized, canonically normalized, or encoded without invalidating the logical target;
- whether new lower-bound witnesses can be generated that remain indistinguishable under the exact ordered logic being studied.

Negative results are first-class outcomes and must be retained with counterexamples.

### PL-DC-5 — Candidate separation mechanisms

Only after PL-DC-0 through PL-DC-4 provide reliable controls may the system promote genuinely new candidate mechanisms.

Candidate-generation sources may include:

- proof-pattern mining;
- automated finite-model search;
- invariant synthesis;
- adversarial ordered-structure generation;
- Forge-driven candidate search;
- LLM proposals;
- algebraic or combinatorial constructions.

Each candidate must pass, in order:

```text
FORMALIZE ASSUMPTIONS
-> SEARCH FOR SMALL COUNTEREXAMPLES
-> TEST ORDER ROBUSTNESS
-> TEST AGAINST KNOWN CFI/WL/GAME CONTROLS
-> ATTEMPT FORMAL PROOF
-> MINIMIZE ASSUMPTIONS
-> CLASSIFY RESULT
```

No candidate may be described as evidence for `P != NP` merely because it defeats a bounded game, a fixed WL dimension, FPC on unordered structures, or a finite search battery.

## Initial implementation boundary

ProofLab should reuse mathlib's existing first-order model-theory substrate where compatible. The initial repository audit found usable first-order languages, semantics, graph model theory, ordered model theory, and polynomial-time Turing-machine infrastructure, but no ready-made descriptive ESO/LFP layer was identified by the code search performed for this programme.

Therefore the first implementation task is not a P-versus-NP attack. It is a small, tested descriptive-logic layer that can state and evaluate the known control examples correctly.

## Primary references

- Ronald Fagin, *Generalized First-Order Spectra and Polynomial-Time Recognizable Sets*, 1974.
- Moshe Y. Vardi, *The Complexity of Relational Query Languages*, 1982.
- Neil Immerman, polynomial-time / least-fixed-point characterizations on ordered finite structures, 1980s; see also *Descriptive Complexity*.
- Jin-Yi Cai, Martin Fuerer, Neil Immerman, *An Optimal Lower Bound on the Number of Variables for Graph Identification*, Combinatorica 12(4), 389-410, 1992, DOI 10.1007/BF01305232.

## Success condition

PL-DC is successful before any major open problem is solved if it becomes a trustworthy, reproducible, machine-checkable research bench for finite descriptive complexity that:

- reproduces known expressibility and inexpressibility controls;
- exposes false proof strategies quickly;
- keeps ordered/unordered distinctions explicit;
- turns computational witnesses into formal proof obligations;
- records negative results and counterexamples;
- makes any future original theorem auditable from conjecture through kernel verification.
