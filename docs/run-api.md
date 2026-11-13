# Run API

## Preview

`POST /api/v1/runs/preflight` validates the target, scenario, and workload. It returns a normalized origin, scenario hash, workload preview, and synthetic-traffic warning. It does not create a run or start k6.

## Create

`POST /api/v1/runs` accepts the same scenario and workload plus project, environment, and scenario IDs. The request must contain `confirmed: true`. The API validates the request again and persists a queued run with an immutable scenario snapshot and hash.

The response contains the run ID and `queued` status. Execution is a separate orchestration step, so a browser retry cannot accidentally start multiple k6 processes.

## Execute locally

`POST /api/v1/runs/{run_id}/execute` accepts a queued run, transitions it to `running`, invokes the registered k6 script, writes raw and summary artifacts beneath `ARTIFACT_ROOT`, builds the versioned report, and stores the terminal result. A nonzero exit, timeout, missing summary, or invalid report leaves the run failed.

Automated tests use the fake k6 fixture and a local target. Azure execution requires a separately reviewed operator action.

