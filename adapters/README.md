# ProofLab adapters

Future SciRust / TDI / Riemann bridges live here as thin, tested ingest surfaces.

## Current

- **Riemann stub (PL-2.0)**: library path in `prooflab-core::bench_adapter`
  (`ingest_riemann_stub`). Fixture manifest:
  `experiments/PL-2.0/fixtures/riemann_stub_manifest.json`.
  Emits `Observation` / `EvidenceClaim` only; epistemic labels
  (`exact`, `numerical`, `conjecture`, `formal_asymptotic`) are preserved
  and never upgraded to `PROVED`.

No live network ingest. Lean remains the sole `PROVED` authority.
