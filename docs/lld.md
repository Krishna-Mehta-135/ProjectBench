# Low-level design

## API modules

- `policy`: validates HTTPS origins, allowlists, workload caps, and stop-condition inputs.
- `domain`: immutable scenario snapshots, run states, metric summaries, and report schema.
- `runner`: a process adapter that accepts a validated typed request and never a shell command.
- `storage`: repository interfaces for metadata and artifacts; PostgreSQL and filesystem implementations are replaceable.
- `http`: Axum handlers that translate typed domain errors into stable JSON responses.

## Invariants

1. A run stores a scenario snapshot and SHA-256 hash at creation time.
2. A report identifies its target origin, workload, UTC window, metric definitions, and limitations.
3. A target must be HTTPS, have no path, and match an explicit allowlist entry.
4. Failed, canceled, or partial runs are retained and cannot be presented as passing runs.
5. Secrets never enter scenario files, report JSON, logs, or Git.
6. Run acquisition is an atomic queued-to-running transition; a run cannot be acquired twice.

## Extension points

`RunReport` is versioned independently from database migrations. Artifact storage is behind a kind/path/hash reference. Future AI events are separate from HTTP metric summaries and can link to a run or trace without adding nullable AI columns to every run.

Run execution uses a PostgreSQL acquisition lease. The worker claims a queued run with one conditional update, increments its attempt count, and records a start time. A recovery sweep marks running runs whose lease has expired as failed with a retained reason; replay is an explicit operator decision.

