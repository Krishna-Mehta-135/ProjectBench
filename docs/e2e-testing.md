# Local end-to-end verification

The local verification path uses a fake `k6` executable and never calls Canvassync, Knowdex, Azure, or any external target.

```sh
cargo fmt --all -- --check
cargo test --workspace --all-features
```

The report tests use `tests/fixtures/k6-summary.json`. The fake executable under `tests/fixtures/bin/k6` is only a process fixture; it produces a deterministic local artifact and does not generate network traffic.

Before relying on a local result, confirm that dependencies resolved successfully. A formatting pass is not a substitute for compiled tests.

## PostgreSQL-backed verification

The Rust quality job starts a disposable PostgreSQL 16 service and sets `DATABASE_URL` for SQLx integration tests. Storage tests apply the repository migrations, seed isolated project/environment/scenario/run rows, and verify that concurrent acquisition has one winner and stale running leases are retained as failed.

These tests use database fixtures only. They do not start k6 against Canvassync, Azure, or any external target.

## Full fake-k6 workflow

The API test `fake_k6_covers_success_failure_cancellation_and_artifacts` drives the real HTTP routes with disposable PostgreSQL and the fixture executable. It verifies a successful report and both terminal non-passing paths: retained runner failure and explicit cancellation. The fixture writes deterministic raw JSONL and summary JSON, and its failure/sleep modes are local process controls only.

The test also verifies that the report's run ID matches the persisted run and that raw artifacts are retrieved through the persisted artifact reference rather than a caller-supplied path.
