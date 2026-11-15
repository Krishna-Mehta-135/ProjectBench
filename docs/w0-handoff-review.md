# W0 handoff comparison

Compared the uncommitted `../ProjectBench-handoff` changes with its `main` on 2026-11-15. These edits are **not** in the fresh ProjectBench checkout:

- `apps/api/src/lib.rs`: validates a run's persisted environment and scenario before queueing it, and passes the normalized target origin to storage. One test fixture gains an explicit allowed origin.
- `apps/api/src/storage.rs`: adds configuration errors, checks project ownership, environment enabled state and origin allowlist, and matches the scenario snapshot against its stored version. It stores the normalized target origin and restricts completion/failure updates to running rows. Its test fixture also gains an allowed origin.

This is unfinished control-plane work, outside the claims-first W0 scope. The handoff folder is intact. Review these changes separately before deciding whether any part belongs in a later package.
