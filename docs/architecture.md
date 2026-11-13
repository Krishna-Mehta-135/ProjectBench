# Metrics architecture

Metrics starts as a self-hostable modular monolith.

- **API:** Rust service using Axum.
- **Web UI:** TypeScript client kept separate from the API.
- **Metadata:** PostgreSQL stores projects, environments, scenarios, and run metadata.
- **Runner:** k6 runs as a controlled external process; Metrics does not implement a load generator.
- **Artifacts:** raw k6 output and generated reports use a storage interface, initially backed by local disk.
- **Deployment:** Docker-based local development with an Azure deployment path documented after the local foundation is complete.

The first runner is intentionally single-process. A broker, distributed runner fleet, and dedicated time-series database are deferred until measurements justify them.

Synthetic k6 results, real-user traffic, infrastructure telemetry, and future AI telemetry remain separate data categories. Reports must preserve target, workload, scenario version, time window, metric definitions, and limitations.
