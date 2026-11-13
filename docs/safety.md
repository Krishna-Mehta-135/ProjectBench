# Safety model

The first baseline is intentionally conservative because Canvassync shares a small Azure application host with other services.

- Targets are explicit HTTPS origins and must be allowlisted per environment.
- Workloads are previewed before execution and capped by VUs, duration, and hard runtime.
- The first profile uses one constant VU, 60 seconds, and two seconds of think time.
- Credentials are injected at runtime from environment variables and never stored in scenarios, reports, or logs.
- The scenario signs in with a dedicated test account and performs only room-list and seeded-room read operations.
- Signup, writes, AI calls, collaboration traffic, and redirecting to a different host are excluded.
- A live run requires operator review of target, account/test data, workload, and stop conditions. This repository phase does not execute the Azure run.

