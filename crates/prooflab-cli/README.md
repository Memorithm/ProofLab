# prooflab-cli

Thin user/agent entry point over existing ProofLab library APIs.

## Commands

```bash
cargo run -p prooflab-cli -- falsify
cargo run -p prooflab-cli -- reproduce --artifact artifact.json --lock lock.json
cargo run -p prooflab-cli -- reproduce --artifact artifact.json --lock lock.json \
  --formal formal.json --source path/to/File.lean
cargo run -p prooflab-cli -- verify-corpus --repo-root .
cargo run -p prooflab-cli -- minimize --repo-root .
cargo run -p prooflab-cli -- inspect --kind riemann-stub-manifest \
  --input experiments/PL-2.0/fixtures/riemann_stub_manifest.json
cargo run -p prooflab-cli -- inspect --kind tdi-stub-manifest \
  --input experiments/PL-2.0/fixtures/tdi_stub_manifest.json
cargo run -p prooflab-cli -- inspect --kind observation --input observation.json
```

## Trust notes

- Lean remains the sole `PROVED` authority.
- `falsify` records `FALSIFIED` only and never invokes Lean.
- `reproduce` success is integrity/environment confirmation (optional Lean re-check); it does not authorize `PROVED`.
- `minimize` records `Removable` / `RemovalRejected` only; rejection is not a necessity claim.
- `inspect` is read-only over PL-2.0 evidence / stub-manifest JSON; it never seals `PROVED` (even for accepting `KernelResult`).
