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

For the disposable Compose deployment, run `METRICS_OPERATOR_TOKEN='…' docker compose up -d`. PostgreSQL uses the named `metrics-postgres` volume and API artifacts use `metrics-artifacts`; both survive container recreation until explicitly removed. The sample database password is local-only and must be replaced before any shared deployment.

The API binds to `0.0.0.0:8080` inside the container/process. The dashboard can be started separately with `cd apps/web && npm ci && npm run dev`.

## Operator authentication

Set `METRICS_OPERATOR_TOKEN` before binding the API beyond a trusted local-only process:

```sh
export METRICS_OPERATOR_TOKEN='use-a-secret-from-your-secret-manager'
```

When set, every API route except `/health` requires `Authorization: Bearer <token>`. This protects configuration writes, run execution and cancellation, reports, and artifact downloads. If the variable is absent, the API remains in local-development mode and is not suitable for an exposed deployment. The token is never persisted or included in reports and logs.

## Database

When `DATABASE_URL` is configured, the API applies the tracked SQLx migrations during startup. The Compose PostgreSQL service therefore only needs to be healthy before the API starts; do not apply the same migrations manually.

## Backup, restore, and upgrades

Back up PostgreSQL and the artifact volume together. A database-only restore can leave metadata pointing at missing bytes, while an artifact-only restore can leave inaccessible files. A minimal nonproduction backup is:

```sh
docker compose exec -T postgres pg_dump -U metrics -d metrics > metrics.sql
docker run --rm -v projectbench_metrics-artifacts:/data -v "$PWD:/backup" alpine tar czf /backup/metrics-artifacts.tgz -C /data .
```

Restore into a disposable stack first, then start the API so SQLx applies additive migrations. Keep the previous image and database/artifact backups until the restored health check and a hash-verified artifact download succeed. There is currently no automatic artifact deletion job; retention timestamps are recorded as metadata and operational cleanup must be explicitly scheduled after a reviewed policy exists.

The API `/health` endpoint is a liveness probe. Database connectivity and migrations are validated during startup; a process that cannot connect or migrate exits rather than presenting a ready control plane.

## Nonsecret configuration transfer

Registry definitions can be exported from `GET /api/v1/projects/{id}/export` and imported with `POST /api/v1/projects/import`. The document contains only project metadata, HTTPS origin allowlists, scenario paths, route/config snapshots, and hashes; it must not contain passwords, tokens, cookies, or customer data. Import creates a new project identity transactionally and rejects invalid origins, traversal paths, duplicate versions, secret-like keys, or mismatched config hashes. A successful import still requires operator review of the target, credentials, seeded data, workload, and stop criteria before any live run.

## Resource and logging controls

Run the API as a non-root container user in a production image, place PostgreSQL and the artifact volume on private networks/storage, cap CPU/memory and concurrent runs at the deployment layer, and send only redacted application logs to a central sink. Never put `METRICS_OPERATOR_TOKEN`, runtime credentials, cookies, or raw customer content in logs or backups without an approved retention policy.

## Canvassync approval gate

Before any non-local run, confirm the exact HTTPS origin, dedicated test account, seeded room, VU count, duration, think time, hard runtime, and stop conditions. The first proposed workload remains one VU for 60 seconds with two seconds of think time. Do not store any of those secrets in Git.

