# Development execution ingestion

`BenchExecutionManifest` adds executed-development provenance above the existing
`BenchExportManifest`: exact producer repository/revision, protocol bytes, input
bytes, development split membership, output bytes, CI run and attempt. It reuses
ProofLab's canonical serialization and SHA-256 primitives adapted from SciRust.
It creates only `Observation` and `EvidenceClaim`; Lean alone may seal a proof.

The importer rejects source changes, duplicate/missing source inventory,
unauthorized splits, final/test split labels, output substitution, wrong CI/run
identity, oversized records and byte budgets. The command reads bounded snapshots
and never launches producer code or fetches a network resource.

```bash
cargo run --locked -p prooflab-cli --bin prooflab-ingest-execution -- \
  --claim claim.json --manifest execution.json \
  --expected-execution-id "$INDEPENDENT_EXECUTION_ID" \
  --allow-development-split development/panel \
  --sources /trusted/staged-development-artifacts \
  --entry-id output --payload /trusted/staged-development-artifacts/output.json
```

The expected content identity and allowed split names are independent caller
policy. A pin copied from the submitted record is not authentication. Keep the
staging root private and immutable while importing; this path does not claim
hostile-writer filesystem isolation or an OS sandbox.

## Existing owner trust contract

SciRust-Verify owns detached dossier authenticity at integrated revision
`662a30dc774dd19c68a79bed3889036a91930123`, including an external allowed-signers
root. Its `authenticate` command binds the exact sealed bundle to its principal.
Inspect the authenticated dossier's sealed execution attachment and derive the
expected execution identity only after the independent owner policy accepts it.
Integrity, authenticity, execution truth and scientific qualification remain
distinct. This importer does not duplicate OpenSSH signature or trust-root code.

## Audit status

This consumer slice uses a real archived Riemann CI log: source
`d7f0b70feb515a3f3a80cd681d820510bc2ed938`, run `37193369605`, job
`111410052824`. The complete connector-decoded log is retained with SHA-256,
and source/protocol/development membership bytes come from that exact checkout.
The examples and CI authenticate its imported record through the existing
SciRust-Verify command at its exact integrated pin, then ingest exact bytes.

The independent ephemeral signer policy qualifies this import path; it is not
a production organization publisher policy or a new Riemann execution. The
signature authenticates the imported record and does not establish scientific
truth. This slice does not implement a live TDI transport. The audit finding
remains open until the complete candidate and integrated consumer CI succeed
and the recorded scope satisfies the acceptance criteria.
No historical holdout is read or promoted by these commands.
