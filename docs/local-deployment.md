# Local deployment

## Prerequisites

- Rust stable and Cargo
- Node.js 22 and npm
- Docker Desktop (optional, for PostgreSQL)
- k6 only when a reviewed local scenario is ready

## Start dependencies

```sh
docker compose up -d postgres
cargo run -p metrics-api
```

The API binds to `0.0.0.0:8080` inside the container/process. The dashboard can be started separately with `cd apps/web && npm ci && npm run dev`.

## Database

When `DATABASE_URL` is configured, the API applies the tracked SQLx migrations during startup. The Compose PostgreSQL service therefore only needs to be healthy before the API starts; do not apply the same migrations manually.

## Canvassync approval gate

Before any non-local run, confirm the exact HTTPS origin, dedicated test account, seeded room, VU count, duration, think time, hard runtime, and stop conditions. The first proposed workload remains one VU for 60 seconds with two seconds of think time. Do not store any of those secrets in Git.

