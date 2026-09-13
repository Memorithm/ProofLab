# PL-CLI — Thin agent/UX entry point

Expose existing ProofLab library APIs through a single `prooflab` executable so
agents and local operators can run:

- `reproduce` — integrity + environment lock binding, optional Lean re-verify;
- `falsify` — PL-1.1 controlled false-conjecture battery (no Lean);
- `verify-corpus` — PL-1.0 known-theorem corpus through `LeanKernel`;
- `minimize` — PL-1.2 assumption minimization through `LeanKernel`.

## Control

1. CLI lives in `crates/prooflab-cli` and depends only on `prooflab-core` /
   `prooflab-lean`.
2. Commands call library functions; they do not reimplement trust logic.
3. JSON reports record outcomes without claiming novelty or necessity.
4. Lean remains the sole `PROVED` authority. Falsify records `FALSIFIED` only.
   Reproduce success is not proof authorization. Minimization rejection is never
   reported as necessity.

## Success criteria

- `falsify` and core `reproduce` unit tests pass without Lean;
- clap parses all four primary commands;
- `cargo fmt` / `clippy -D warnings` / `test -p prooflab-cli` are green;
- docs mention the CLI without promoting it to a proof kernel.

## Non-claims

Shipping the CLI does not establish discovery capability, complete falsification,
or any new theorem. It is an orchestration UX surface over already-tested APIs.
