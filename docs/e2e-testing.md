# Local end-to-end verification

The local verification path uses a fake `k6` executable and never calls Canvassync, Knowdex, Azure, or any external target.

```sh
cargo fmt --all -- --check
cargo test --workspace --all-features
```

The report tests use `tests/fixtures/k6-summary.json`. The fake executable under `tests/fixtures/bin/k6` is only a process fixture; it produces a deterministic local artifact and does not generate network traffic.

Before relying on a local result, confirm that dependencies resolved successfully. A formatting pass is not a substitute for compiled tests.

