# Metrics

Metrics is a project performance and observability platform for stress-testing Canvassync, Knowdex, and future applications. It turns repeatable k6 runs and application telemetry into comparable, shareable evidence about reliability and performance.

See [PRD.md](PRD.md) for the product requirements and proposed implementation plan.

## Development

The architecture and implementation boundaries are documented in [docs/hld.md](docs/hld.md), [docs/lld.md](docs/lld.md), and [docs/safety.md](docs/safety.md). Start the API locally with `cargo run -p metrics-api`; it binds to `127.0.0.1:8080` and exposes `/health` plus a guarded preflight endpoint.

The first Canvassync profile is review-only. It requires runtime-injected credentials and a seeded room ID; no Azure load test is run by local development or CI.
