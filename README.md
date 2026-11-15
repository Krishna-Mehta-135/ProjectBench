# ProjectBench

ProjectBench turns an engineering claim into a reproducible experiment with a verdict and a public evidence page. The current implementation contains an early k6 run control plane; the claims-first work is tracked in [the week-one plan](docs/plan-v2-claims-first.md).

## Development

Run `cargo run -p metrics-api` to start the private API on `127.0.0.1:8080`. Set `METRICS_BIND_ADDR` to change the listener. A non-loopback address requires `METRICS_OPERATOR_TOKEN`.

The existing k6 profile is review-only. Benchmarks must target local Docker Compose, never production sites.
