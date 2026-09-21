# ProofLab adapters

Future SciRust / TDI / Riemann bridges live here as thin, tested ingest surfaces.

## Current

- **Riemann stub (PL-2.0)**: library path in `prooflab-core::bench_adapter`
  (`ingest_riemann_stub`). Fixture manifest:
  `experiments/PL-2.0/fixtures/riemann_stub_manifest.json`.
  Emits `Observation` / `EvidenceClaim` only; epistemic labels
  (`exact`, `numerical`, `conjecture`, `formal_asymptotic`) are preserved
  and never upgraded to `PROVED`.
- **TDI stub (PL-2.0)**: parallel path in the same module (`ingest_tdi_stub`).
  Fixture: `experiments/PL-2.0/fixtures/tdi_stub_manifest.json`. Same PL-C3
  label vocabulary and trust rules; observations use a `tdi-stub://` URI
  scheme distinct from `riemann-stub://`.

No live network ingest. Lean remains the sole `PROVED` authority.
