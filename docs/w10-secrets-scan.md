# W10 ProjectBench secrets scan

Date: 2026-11-18. Scope: the ProjectBench Git repository only.

## Method

- Confirmed the checkout is not shallow and scanned reachable blobs from all local Git refs: 97 commits and 294 unique blobs.
- Ran a local pattern scan for private-key headers, AWS access-key IDs, GitHub tokens, Slack tokens, Google API keys, JWT-shaped values, and non-placeholder credential assignments. Output was restricted to file/line/pattern names; no candidate values were printed.
- Reviewed the committed runtime/configuration files: `.env.example`, `docker-compose.yml`, and `config/projects/*`.

## Findings

- The pattern scan found no candidates in reachable history.
- `.env.example` and Compose use the intentionally fake `projectbench-demo-token` and `metrics-local-only` credentials. They are for the isolated local demo, not production credentials. Host ports are loopback-bound.
- Project examples contain production hostnames but their environments are disabled. The YAML files contain environment variable names as secret references, not secret values. The notes explicitly prohibit running the scenarios until target and workload approval.
- No `gitleaks` or `trufflehog` executable was available. The regular-expression scan is a useful check, not proof that every kind of secret is absent. This report does not replace an owner review or secret-provider audit.

## Owner follow-up

Keep the demo credentials limited to the local Compose demo. Before enabling any project environment, supply credentials through the approved runtime secret mechanism and review the target and workload. If a real credential is later found in history, revoke it first, then follow the repository's history-removal process.
