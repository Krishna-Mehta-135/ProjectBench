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

The API binds to `127.0.0.1:8080`. The dashboard can be started separately with `cd apps/web && npm install && npm run dev`.

## Database

Apply `migrations/001_initial.sql` with the PostgreSQL client configured for the local database. Migration execution is deliberately separate from application startup until the migration runner is added.

## Canvassync approval gate

Before any non-local run, confirm the exact HTTPS origin, dedicated test account, seeded room, VU count, duration, think time, hard runtime, and stop conditions. The first proposed workload remains one VU for 60 seconds with two seconds of think time. Do not store any of those secrets in Git.

