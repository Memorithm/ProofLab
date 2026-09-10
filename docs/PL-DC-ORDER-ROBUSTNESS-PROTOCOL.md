# PL-DC — Order-Robustness Protocol

## Purpose

This protocol governs PL-DC-4 experiments and proof obligations involving linear order, symmetry breaking, canonization, or order-independent auxiliary choices.

It exists to prevent three materially different settings from being conflated:

1. **Built-in ordered structures.** The linear order is part of the input structure. This is the setting relevant to the Immerman–Vardi characterization of polynomial time by least-fixed-point logic on ordered finite structures.
2. **Order-invariant logic.** A formula may consult an auxiliary linear order, but its truth value must be independent of which order is supplied. This is a distinct semantic condition and must not be substituted for built-in order.
3. **Definable canonization or choice.** A logic or algorithm constructs a canonical representation, definable preorder, orbit choice, or related symmetry-breaking object. Success or failure here is evidence about canonization machinery, not automatically about expressive power on already ordered structures.

## Stage gate

Every PL-DC-4 claim, experiment, generated witness, and candidate theorem must declare exactly one of the three settings above before execution.

A result is rejected as ambiguous if it only says that a construction is "robust to order" without specifying which semantics is meant.

## Built-in-order track

This is the track directly relevant to the ordered FO(LFP) versus ESO formulation.

For a candidate indistinguishability mechanism on unordered structures, the default falsification test is:

```text
UNORDERED PAIR
-> ADD EXPLICIT LINEAR ORDERS
-> SEARCH FOR AN ORDER PAIR THAT BREAKS THE INVARIANT
-> IF FOUND, RECORD COUNTEREXAMPLE AND STOP
-> OTHERWISE QUANTIFY THE SURVIVING CLAIM PRECISELY
```

Passing finitely many order expansions is only finite evidence. It does not establish survival under all orders.

For a property already decidable in polynomial time on ordered structures, a CFI-style finite indistinguishability result against a restricted oracle must not be described as evidence that the property escapes FO(LFP). The restricted oracle and the full logic must remain separate objects.

## Order-invariant track

An order-invariant sentence is allowed to use an auxiliary order only when its truth value is independent of the chosen linear order.

The executable requirement is therefore stronger than testing one canonical order. For a finite structure `A`, a bounded experiment should evaluate a candidate over an explicitly declared family of orders and fail immediately if two tested orders yield different truth values.

A bounded pass is still not a proof of order invariance over all orders.

Recent finite-model-theory work provides useful controls for this distinction. Grange (CSL 2023) studies order-invariant two-variable first-order logic and counting on bounded-degree classes. Ghasemi and Grange (MFCS 2026) study order-invariant cluster first-order logic and explicitly emphasize that adding an order can reveal distinctions otherwise hidden by locality.

## Canonization and symmetric-choice track

Canonization, interpretations, and symmetry-aware choice mechanisms are useful PL-DC calibration targets because they expose how a logic handles arbitrary choices without silently fixing vertex identities.

Lichter (ICALP 2023) shows that witnessed symmetric choice, interpretations, and CFI constructions interact nontrivially in fixed-point logic with counting. In particular, CFI remains useful as a benchmark for canonization and interpretation machinery even when it is not itself a candidate separation of ordered FO(LFP) from ESO.

PL-DC should therefore record separately:

- whether a class can be canonized;
- whether a restricted logic distinguishes a CFI pair;
- whether a construction survives arbitrary order expansion;
- whether the resulting property is already known to be polynomial-time decidable on ordered structures.

No implication may be inserted between these statements without a separately formalized theorem.

## Required experiment metadata

Every PL-DC-4 executable result should record:

- semantic track: `built-in-order`, `order-invariant`, or `canonization-choice`;
- exact logic/oracle under test;
- ordered or unordered input vocabulary;
- source structure identifiers;
- order-generation method and number of tested orders when applicable;
- whether the test is exhaustive for that finite carrier;
- deterministic seed/provenance for generated candidates;
- first counterexample found, if any;
- explicit statement that finite survival is not a universal theorem.

## Promotion rules

A candidate may advance from finite evidence to a formal proof obligation only if:

1. its semantic track is unambiguous;
2. its ordered/unordered assumptions are explicit;
3. no tested order expansion falsifies it;
4. known controls such as EF, pebble, bijective-pebble, WL, and CFI behave as expected for the stated fragment;
5. the universal quantifiers over structures/orders required by the claim are represented in the formal statement rather than inferred from finite enumeration.

Only the configured Lean kernel can promote the resulting formal artifact to `PROVED`.

## Primary references

- Neil Immerman and Moshe Y. Vardi, polynomial-time / least-fixed-point characterizations on ordered finite structures.
- Julien Grange, *Order-Invariance in the Two-Variable Fragment of First-Order Logic*, CSL 2023, DOI 10.4230/LIPIcs.CSL.2023.23.
- Fatemeh Ghasemi and Julien Grange, *Order-Invariant Cluster First-Order Logic on Graph Classes of Bounded Degree*, MFCS 2026, DOI 10.4230/LIPIcs.MFCS.2026.71.
- Moritz Lichter, *Witnessed Symmetric Choice and Interpretations in Fixed-Point Logic with Counting*, ICALP 2023, DOI 10.4230/LIPIcs.ICALP.2023.133.
- Jin-Yi Cai, Martin Fürer, and Neil Immerman, *An Optimal Lower Bound on the Number of Variables for Graph Identification*, Combinatorica 12(4), 389–410, 1992, DOI 10.1007/BF01305232.
