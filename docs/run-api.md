# Run API

## Preview

`POST /api/v1/runs/preflight` validates the target, scenario, and workload. It returns a normalized origin, scenario hash, workload preview, and synthetic-traffic warning. It does not create a run or start k6.

## Create

`POST /api/v1/runs` accepts the same scenario and workload plus project, environment, and scenario IDs. The request must contain `confirmed: true`. The API validates the request again and persists a queued run with an immutable scenario snapshot and hash.

The response contains the run ID and `queued` status. Execution is a separate orchestration step, so a browser retry cannot accidentally start multiple k6 processes.

## Execute locally

`POST /api/v1/runs/{run_id}/execute` accepts a queued run, transitions it to `running`, invokes the registered k6 script, writes raw and summary artifacts beneath `ARTIFACT_ROOT`, builds the versioned report, and stores the terminal result. A nonzero exit, timeout, missing summary, or invalid report leaves the run failed.

`POST /api/v1/runs/{run_id}/cancel` cancels a queued run immediately or signals its owned k6 process to terminate when it is running. Canceled runs remain persisted with `status: canceled`, `finished_at`, and `failure_reason: canceled by operator`; any partial artifacts remain available for operator inspection. Cancellation is distinct from runner failure and stale-lease recovery.

Automated tests use the fake k6 fixture and a local target. Azure execution requires a separately reviewed operator action.

Execution acquisition is atomic. Only a queued run can be claimed; concurrent requests receive a conflict after the first claim. Runner failures are retained with a failure reason. A recovery sweep can mark stale running leases failed without replaying them.

## Artifacts

`GET /api/v1/runs/{run_id}/artifacts` lists the report's retained artifact references. `GET /api/v1/runs/{run_id}/artifacts/{kind}` streams a referenced summary or raw JSONL artifact. The API never accepts a caller-supplied filesystem path; it resolves the kind from the persisted report and applies the local artifact store's path-safety checks.

## Lifecycle metadata

Run list and detail responses include `created_at`, `started_at`, `finished_at`, and `failure_reason`. Timestamps are PostgreSQL UTC timestamps serialized as RFC 3339 values. Failure reasons are bounded operational messages; command lines, environment variables, and credentials are never returned.


## Dashboard behavior

The dashboard exposes cancellation only for queued and running runs, asks for explicit operator confirmation, then refreshes the run list and selected detail. Terminal failed, canceled, and aborted runs remain visible with their lifecycle metadata and do not receive cancellation controls.
