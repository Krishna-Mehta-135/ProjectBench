# ProjectBench code tour

ProjectBench connects a claim to a repeatable workload, validated result documents, a deterministic verdict, and an allowlisted evidence document. The API and PostgreSQL are local operator tools; the static site consumes published JSON and has no write controls. The walkthrough below follows one request through those boundaries.

## Request and storage path

1. `apps/api/src/main.rs` reads the bind address and starts the service. It refuses a non-loopback bind unless an operator token is configured, so the default API stays on the local machine.
2. `apps/api/src/lib.rs` defines shared application state, routes, health/readiness endpoints, operator-token middleware, and the typed API error mapping. Route handlers are split into focused modules instead of placing all database and HTTP logic in one file.
3. `apps/api/src/project_routes.rs` handles project, environment, scenario, registry import/export, and baseline configuration. It validates registry input and rejects secret-looking keys in imported configuration. `apps/api/src/api_types.rs` contains the request and response shapes used by these routes.
4. `apps/api/src/policy.rs` validates a run request against the selected scenario and enabled environment, checks origin and workload limits, and hashes the scenario snapshot. Keeping this logic independent of HTTP and PostgreSQL makes the safety rules reviewable.
5. `apps/api/src/run_query_routes.rs` serves run creation, listing, comparison, reports, time series, and artifact queries. `apps/api/src/run_execution_routes.rs` handles preflight, execution, cancellation, and audit-event queries. The route split follows the query/execution boundary.
6. `apps/api/src/orchestration.rs` enforces confirmation before queueing and coordinates state transitions and report ingestion. `apps/api/src/lifecycle.rs` is the small state-transition rule table; invalid transitions return an error instead of silently changing status.
7. `apps/api/src/storage.rs` defines `PgRepository` and storage errors. SQL is divided among `storage_projects.rs`, `storage_execution.rs`, and `storage_run_queries.rs` so project setup, run ownership/execution, and run reads remain separate. `storage_tests.rs` covers persistence invariants such as single-winner queue acquisition and terminal-state protection.
8. `apps/api/src/runner.rs` builds a constrained k6 command and executes it without shell interpolation. `apps/api/src/report.rs` parses the k6 summary and derives timing and metric summaries. `apps/api/src/report_types.rs` defines the report/status model; `apps/api/src/artifacts.rs` stores local output files and their metadata.
9. `apps/api/src/timeseries.rs` parses k6 JSONL points into bounded time buckets for charting. It keeps parsing and aggregation separate from the API response layer.

## Claims and evidence path

1. `apps/api/src/claims.rs` parses the bundled YAML claims ledger and checks required fields and slugs. `apps/api/src/claims_api.rs` exposes claims, ingested results, result sets, verdict history, and an explicit publish action. `apps/api/src/claims_storage.rs` persists result documents and queries their history.
2. `apps/api/src/results.rs` validates schema-v1 result documents and metric/environment data before they enter storage.
3. `apps/api/src/verdict.rs` summarizes repetitions and applies fixed rules for comparative and behavior claims. It checks minimum repetitions, environment equality, generator CPU, range width, direction, target, and tolerance. It also produces the outcome, next step, and suggested wording from templates; it does not call a model.
4. `apps/api/src/publish.rs` constructs a separate allowlisted evidence shape and writes claim documents plus the evidence index. This is intentionally not a serialization of database rows, which may contain private IDs or operator fields.
5. `apps/web/src/PublicSite.tsx` and `apps/web/src/entry.tsx` render the static public view from evidence JSON. `apps/web/src/main.tsx` hosts the local operator UI. `apps/web/src/api.ts` contains API calls; `apps/web/src/runPresentation.ts` maps stored run data to display labels. The public site is read-only.

## Other entry points and tests

- `apps/api/src/lib_tests.rs` covers HTTP and end-to-end execution paths; `verdict_tests.rs`, `storage_tests.rs`, and the module-local test blocks cover rules near their implementation.
- `apps/demo/server.mjs` is a small local sample endpoint. `apps/demo/harness.mjs` makes real HTTP observations, ingests them, and reports the resulting verdict for `make demo`.
- `scripts/ingest.mjs` posts result files to the local API. `k6/scenarios/` contains the older HTTP read scenarios; these are not the W3/W4 WebSocket experiment harnesses and they do not run against production by default.
- `migrations/` versions the PostgreSQL schema. `docker-compose.yml` binds host ports to loopback and provides an isolated project name through the Make targets.

## Interview questions

**Why is publishing a separate transformation instead of returning stored rows?**

Stored claims and results contain operator fields. `publish.rs` builds only the documented public fields, and tests check that excluded data does not cross the boundary. A public static build can then serve the JSON without API credentials.

**How does a comparative claim become inconclusive?**

The verdict needs enough repetitions for both variants, matching environments, and generator CPU at or below the configured cap. It compares medians and ranges; if an observed range is wider than the median gap, the measured difference is not separated from the run-to-run spread and the result is inconclusive.

**How does the service avoid launching an arbitrary command or target?**

Run policy validates the target against an explicitly enabled environment and checks the scenario's workload caps. The runner constructs a fixed k6 command with argument values rather than invoking a shell command string. Execution also requires an explicit confirmation before queueing.

**Why does the API default to loopback?**

It is an operator API backed by a local database and can read benchmark artifacts. `main.rs` rejects a non-loopback bind unless an operator token is present; the Compose host ports are also bound to `127.0.0.1`.

**How would a new benchmarked project be added?**

Add a schema-v1 harness and claims ledger entries, run it locally, ingest the result files, inspect the deterministic verdict, then review and explicitly publish the allowlisted evidence. The public site only consumes that evidence output.

## Known boundaries

The ProjectBench demo demonstrates the ingest/verdict path with a local HTTP behavior claim. It does not establish CanvasSync or Knowdex performance. The older k6 scenarios are separate from the app-specific WebSocket experiments. Public evidence remains empty until the owner checkpoint and explicit publish action are complete.
