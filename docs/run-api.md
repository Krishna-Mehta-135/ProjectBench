# Run API

## Preview

`POST /api/v1/runs/preflight` validates the target, scenario, and workload. It returns a normalized origin, scenario hash, workload preview, and synthetic-traffic warning. It does not create a run or start k6.

## Create

`POST /api/v1/runs` accepts the same scenario and workload plus project, environment, and scenario IDs. The request must contain `confirmed: true`. The API validates the request again and persists a queued run with an immutable scenario snapshot and hash.

The response contains the run ID and `queued` status. Execution is a separate orchestration step, so a browser retry cannot accidentally start multiple k6 processes.

