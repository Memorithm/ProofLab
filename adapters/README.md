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

- **PL-1.1 falsify bridge**: `prooflab-core::falsify_evidence`
  (`evidence_from_falsification`) maps validated `FalsificationRecord`s into
  typed `Observation` / `EvidenceClaim` with `falsify://pl-1.1/…` provenance.
  Scientific status stays `Falsified`; evidence stays `Observed`; conjecture
  promotion and proof sealing are refused.

No live network ingest. Lean remains the sole `PROVED` authority.

Read-only CLI inspect for these fixtures:

```bash
cargo run -p prooflab-cli -- inspect --kind riemann-stub-manifest \
  --input experiments/PL-2.0/fixtures/riemann_stub_manifest.json
cargo run -p prooflab-cli -- inspect --kind tdi-stub-manifest \
  --input experiments/PL-2.0/fixtures/tdi_stub_manifest.json
```

Inspect never seals `PROVED`.

