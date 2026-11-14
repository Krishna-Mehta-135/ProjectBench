# Run API

## Preview

`POST /api/v1/runs/preflight` validates the target, scenario, and workload. It returns a normalized origin, scenario hash, workload preview, and synthetic-traffic warning. It does not create a run or start k6.

## Create

`POST /api/v1/runs` accepts the same scenario and workload plus project, environment, and scenario IDs. The request must contain `confirmed: true`. The API validates the request again, then proves that the enabled environment and immutable scenario version belong to the selected project, that the normalized target is in the environment's persisted origin allowlist, and that the supplied scenario identity matches the stored version before persisting a queued run.

The response contains the run ID and `queued` status. Execution is a separate orchestration step, so a browser retry cannot accidentally start multiple k6 processes.

## Execute locally

`POST /api/v1/runs/{run_id}/execute` accepts a queued run, transitions it to `running`, invokes the registered k6 script, writes raw and summary artifacts beneath `ARTIFACT_ROOT`, builds the versioned report, and stores the terminal result. A nonzero exit, timeout, missing summary, or invalid report leaves the run failed.

The child process receives a bounded runtime environment containing the persisted target and workload plus explicitly named secret values when configured. The parent environment is cleared; secrets are never command arguments, report fields, logs, or database snapshots.

`POST /api/v1/runs/{run_id}/cancel` cancels a queued run immediately or signals its owned k6 process to terminate when it is running. Canceled runs remain persisted with `status: canceled`, `finished_at`, and `failure_reason: canceled by operator`; any partial artifacts remain available for operator inspection. Cancellation is distinct from runner failure and stale-lease recovery.

Automated tests use the fake k6 fixture and a local target. Azure execution requires a separately reviewed operator action.

Execution acquisition is atomic. Only a queued run can be claimed; concurrent requests receive a conflict after the first claim. Runner failures are retained with a failure reason. A recovery sweep can mark stale running leases failed without replaying them.

## Artifacts

`GET /api/v1/runs/{run_id}/artifacts` lists independently persisted artifact references, including usable partial files from failed or canceled runs. `GET /api/v1/runs/{run_id}/artifacts/{kind}` streams a referenced summary or raw JSONL artifact only after verifying its SHA-256 against metadata. The API never accepts a caller-supplied filesystem path; it resolves the kind from the artifact table and applies the local artifact store's path-safety checks.

## Lifecycle metadata

`GET /api/v1/runs` accepts optional `project_id`, `status`, `limit`, and `offset` query parameters. Results remain a JSON array, default to the newest 50 runs, cap `limit` at 100, and use `created_at DESC, id DESC` ordering for deterministic pages. Status filters accept `queued`, `running`, `completed`, `failed`, `canceled`, or `aborted`; invalid filters and limits are rejected.

## Environment controls

`PATCH /api/v1/projects/{project_id}/environments/{environment_id}` accepts `{ "enabled": false }` to revoke an environment without editing PostgreSQL. The project scope is checked server-side and the updated registry record is returned. Disabling an environment prevents new run creation and causes queued execution to fail the policy revalidation before k6 starts; re-enable it only after the target and workload have been reviewed.

`PATCH /api/v1/projects/{project_id}` accepts a project `name` and optional `description`. It changes descriptive metadata only; scenario versions, run ownership, reports, and artifacts retain their original provenance.

`GET /api/v1/audit-events` returns the newest protected configuration and run-control events. Events contain only action names, resource IDs, safe status metadata, and UTC timestamps; bearer tokens, secret values, raw request bodies, and customer content are never stored. Retention should follow the deployment's approved operational policy.

Run list and detail responses include `created_at`, `started_at`, `finished_at`, and `failure_reason`. Timestamps are PostgreSQL UTC timestamps serialized as RFC 3339 values. Failure reasons are bounded operational messages; command lines, environment variables, and credentials are never returned.

