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
```

## Trust notes

- Lean remains the sole `PROVED` authority.
- `falsify` records `FALSIFIED` only and never invokes Lean.
- `reproduce` success is integrity/environment confirmation (optional Lean re-check); it does not authorize `PROVED`.
- `minimize` records `Removable` / `RemovalRejected` only; rejection is not a necessity claim.
