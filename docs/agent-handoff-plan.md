# Metrics: agent handoff and delivery plan

**Prepared:** 2026-11-14
**Audience:** an implementation agent taking over the Metrics repository
**Status:** planning document based on the source snapshot in this workspace; verify the GitHub branch and CI before editing or claiming completion.

## 1. Mission and success criteria

Build **Metrics**, a private, self-hostable, Grafana-inspired place to produce trustworthy performance evidence for applications we own. The first target is **Canvassync** on Azure; **Knowdex** and future projects should use the same onboarding, execution, report, and comparison workflow. Metrics should eventually measure itself as well. The purpose is to answer what changed, under which workload and deployment, and whether a release regressed. Synthetic k6 traffic, actual user experience, infrastructure signals, and AI events are different kinds of evidence and must retain their identities.

The product's first real success is not simply that a fake process yields a report. A developer should be able to register Canvassync, configure a safe read-only journey, execute a reviewed baseline, inspect summary and time-series results, mark a baseline, run the same scenario again, compare compatible runs, and export evidence with enough provenance to reproduce it. Then Knowdex should be added through configuration, without changing the Metrics application.

Use **Rust/Axum** for the API, **k6** for traffic, **PostgreSQL** for configuration and run metadata, a **TypeScript/React** dashboard, and initially a durable filesystem for raw artifacts. Keep a modular monolith with one runner as the starting point. Add a separate worker, broker, time-series database, or Blob storage only when an operational requirement or measured scale justifies it. AI observability follows the performance MVP; do not make it a prerequisite.

Read [PRD](../PRD.md), [HLD](hld.md), [LLD](lld.md), [safety model](safety.md), [run API](run-api.md), and [report format](report-format.md) before changing contracts. This document records the implementation gap where those documents currently describe intent rather than code.

### Explicit MVP acceptance

1. A protected operator can create or select a project, enabled environment, immutable scenario version, and permitted workload through the UI/API.
2. A preflight shows the exact target, routes, VUs, duration, think time, limits, and warnings. The runner executes only the persisted, approved configuration. Credentials and test data are injected privately at runtime.
3. A run has a reliable lifecycle, stop control, failure reason, retained partial data where available, and a complete report with actual UTC timestamps, scenario/script identity, target, workload, metric definitions, limitations, and threshold outcomes.
4. The dashboard shows request rate, latency, and error time series alongside p50/p90/p95/p99, throughput, error rate, and threshold outcomes. Missing data is visibly different from zero.
5. The operator can mark a completed compatible run as a baseline and compare a second run, with absolute and relative deltas and clear incompatibility warnings.
6. A controlled Canvassync read journey has produced a baseline and a second comparable result. Knowdex can then be onboarded by configuration. Evidence is exportable and can be traced to the exact scenario and target.
7. Tests and CI cover safety, persistence, execution, failure, cancellation, report semantics, comparison, and UI behavior without contacting external targets.

## 2. Source snapshot: what exists today

The workspace contains source files but **no `.git` directory**. This handoff cannot assert the current remote branch, recent PR or issue state, or current CI result. The next agent must verify these through the connected GitHub repository before implementation. The prior conversation reported completed implementation Phases 1–8 and a green `main` at `eed26d0`; treat that as historical context, not a substitute for current verification. The PRD's six delivery milestones are a different numbering scheme from those implementation phases.

| Area | Present in source | What remains / qualification |
| --- | --- | --- |
| API | Axum health, project listing, run preflight/create/list/detail/execute/cancel/report/artifact routes in `apps/api/src/lib.rs` | No project, environment, or scenario write API; no operator authentication. |
| Persistence | SQLx migrations for projects, environments, scenarios, runs, execution attempts and failure reasons; atomic acquisition and stale-run recovery | Current project listing returns an empty environment name and null latest run ID; input IDs and target are not reconciled with stored environment/scenario policy. |
| Runner | Typed k6 process arguments, timeout, cancellation, fake executable tests | Runtime `TARGET_ORIGIN`, credentials, room ID, and think time are not passed explicitly to the child; caller-supplied scenario path reaches the process. |
| Reporting | k6 summary parsing, percentiles, requests, throughput, error rate, one threshold, report and artifact refs | Threshold is hard-coded to 5%; report's own actual start/finish, script digest, and metric definitions are incomplete against its documented contract. Nonzero k6 exit and failed threshold semantics need explicit handling. |
| Artifacts | Filesystem write/read and SHA-256 refs; successful run summary and JSONL are downloadable | Failed/canceled runs without a report have no artifact-listing path; partial files may exist on disk but are not exposed as durable references. |
| Dashboard | Project cards, recent runs, report metrics, threshold labels, artifact links, cancel with confirmation | “New run” is disabled; no setup flow, charts, baseline, comparison, or auth. “Baseline available” currently depends on `latest_run_id`, which repository listing always returns null. |
| Canvassync | Example YAML and read-only k6 script under `config/` and `k6/` | Profile is illustrative; runtime wiring, route validation, target ownership, approved test account/seeded room, and real baseline are not yet demonstrated. |
| Verification | Rust unit/SQLx tests; fake-k6 HTTP workflow; frontend API/presentation tests; GitHub Actions with PostgreSQL | No current live CI verification from this snapshot. CI uses `npm install`; there is no tracked npm lockfile in this snapshot. |
| Deployment | Dockerfile, Compose PostgreSQL/API, local deployment guide | Persistence and operational validation need work; current `main.rs` binds `0.0.0.0:8080`, while README says `127.0.0.1`; SQLx applies migrations on connect despite a guide saying migration is separate. |

### Completed implementation phases (historical grouping)

1. **Foundation:** Rust API, web shell, PRD/HLD/LLD and safety guidance.
2. **Domain and guardrails:** preflight, typed workload/scenario, allowlisted HTTPS target, scenario snapshot/hash.
3. **Local execution:** k6 adapter, raw/summary output, versioned JSON report.
4. **Persistence:** PostgreSQL-backed projects/scenarios/runs and HTTP run flow.
5. **Reliability and access to artifacts:** atomic acquisition, attempts, stale recovery, safe artifact routes, report UI.
6. **Lifecycle:** timestamps/reasons, cancel endpoint and UI foundations, PostgreSQL tests.
7. **End-to-end tests:** fake-k6 success, failure, cancellation and artifact retrieval through HTTP and database.
8. **Operator UI refinement:** cancellation confirmation, status presentation, refresh and frontend tests.

These phases delivered a useful local control-plane prototype. They did **not** close the PRD milestones for real Canvassync evidence, result charts, comparison, Knowdex reuse, or AI telemetry. Earlier percentages such as “60% of MVP” were subjective estimates; use the explicit MVP acceptance list above to measure progress.

## 3. Important implementation risks to verify first

These are source-level observations, not proof that a live run has failed. Turn each confirmed problem into a narrow issue and regression test.

1. **Execution policy binding:** `RunPolicy` is process-global and hardcodes the Canvassync origin. `create_run` accepts arbitrary project/environment/scenario IDs and scenario content; database foreign keys only prove IDs exist, not that they belong together or match the environment's allowlist. `execute_run` trusts the stored script path. Resolve the effective policy and approved script by project/environment/scenario identity on the server; reject cross-project and disabled-environment combinations. Revalidate the approved snapshot at execution.
2. **Child process inputs:** the k6 scenario reads `TARGET_ORIGIN`, `TEST_ACCOUNT_EMAIL`, `TEST_ACCOUNT_PASSWORD`, `SEEDED_ROOM_ID`, `VUS`, `DURATION`, and `THINK_TIME_SECONDS`. The runner currently passes CLI VUs/duration, but no explicit per-run environment. Specify a limited child environment from approved configuration and secret references; validate that the actual script uses the approved origin and never follows off-origin redirects. Do not place secrets in database snapshots, report JSON, logs, Git, or child command arguments.
3. **Terminal-state correctness:** after claiming `running`, filesystem or report errors can return before `fail_run` executes. `cancel_run` and `save_report` do not currently use conditional terminal transitions, so races can overwrite a canceled or recovered run. Cancellation before the in-process sender is registered needs deterministic behavior. Make all completion/failure/cancel transitions conditional and idempotent, and put terminalization around the whole execution lifecycle.
4. **Artifact integrity and partial runs:** record artifacts independently from successful reports, verify content hash on retrieval, and protect against symlink/path substitution. Failure/cancel should expose partial data only when valid and sanitized, marked partial, with retention limits. Raw JSONL can include URL and header details; assume it may be sensitive.
5. **Report semantics:** `build_report` generates a random run ID that the HTTP handler later overwrites; use a single authoritative ID. Define actual elapsed time/throughput, denominator and missing-data behavior, threshold source and status. k6 may exit nonzero on threshold failure while still writing a valid summary; preserve the summary and record threshold failure distinctly from process/infra failure.
6. **Documentation drift:** runtime bind address, migration behavior, incomplete report fields, retained partial artifact claims, and “latest baseline” dashboard copy currently disagree with source. Update docs when a matching behavior is implemented; remove misleading claims immediately.

## 4. Engineering practices and working agreement

### How to work

- Inspect current remote `main`, open issues/PRs, CI, and actual repository history. Read repository instructions. Preserve unrelated work and use a feature branch. If the workspace still has no Git checkout, obtain one through the authorized connected GitHub workflow before claiming a push or PR.
- Work in small vertical slices. Create an issue for each outcome below with scope, constraints, acceptance tests, and dependency links. Use multiple focused commits: failing/characterization test where appropriate, implementation, then docs/UI follow-up. Use a draft PR for reviewable work; link issues and note tests and remaining risks.
- Prefer tests that establish externally observable behavior and failure/race semantics. For core logic, first write a failing test, implement, refactor, and rerun. Do not add superficial tests that simply duplicate implementation. Keep local fake-k6 and disposable PostgreSQL tests deterministic.
- Maintain clean boundaries: domain/policy invariants independent of HTTP, repository transactional operations behind an interface, runner accepts a validated execution spec, artifacts behind storage interface, frontend calls versioned API. Avoid a large handler that owns validation, execution, reporting, and persistence in one function.
- Make changes incrementally and remain compatible with persisted reports; use additive SQL migrations and versioned API/report payloads. Document upgrade/rollback of schema and artifacts. Never silently reinterpret old reports.
- Keep deployment and resource choices simple. A single executor is acceptable if maximum concurrency, queue behavior, ownership, and recovery are explicit. Measure before introducing distributed workers or a new datastore.
- Prefer structured, bounded error codes/messages and typed state transitions. Use UTC for persisted timestamps; validate IDs, rate limits, origin, workload, and script identity at the server boundary. Distinguish unavailable, failed, aborted, canceled, and threshold-failed states.
- Track usability: informative empty states, loading/error feedback, accessible controls, visible target/workload preview, clear synthetic label, and no claim of a baseline unless one has been explicitly selected.

### Gates for each PR

1. Explain the user-visible behavior and affected contract; add a focused test covering happy path and the meaningful failure or concurrency case.
2. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and in `apps/web`, `npm ci`, `npm run build`, `npm test`. Start disposable PostgreSQL for SQLx tests; use the fixture k6. If a check cannot run locally, record the reason and wait for CI evidence.
3. CI must use a committed lockfile and reproducible installs. Pin/update dependencies intentionally, review security updates, and ensure no credentials are required by tests.
4. Update PRD/HLD/LLD/API/report/safety/deployment docs as contracts change. State what was measured, what was simulated, and what has not been run on Azure.
5. Merge only after required CI is green and review findings are resolved. Record the resulting issue closure and merge SHA for the next handoff.

### Safety and scope boundaries

- Automated tests use only local fixtures and disposable databases. Never point CI at Canvassync, Knowdex, or Azure.
- For a real Canvassync baseline, obtain the exact origin, dedicated account, seeded room, environment readiness, workload and stop criteria from the operator. The example profile proposes one VU for 60 seconds with two seconds of think time; the actual approval and run must use the reviewed values. Limit the journey to sign-in and read-only room endpoints. Prevent redirects and network access to unexpected origins at the runner boundary where feasible.
- Do not manufacture a baseline, fabricate metrics, or call simulated results production evidence. Never store tokens, passwords, cookies, raw AI prompts, or customer content in Git, reports, or logs. Protect raw artifact access and set retention.
- This handoff authorizes planning and ordinary local implementation. A live test against an external deployment requires the operator's exact target and credentials/test data. Publishing or deploying should follow the user's existing authorization and repository rules.

## 5. Proposed implementation sequence

The next phases below are **forward delivery slices**, separate from the prior eight implementation phases. Open the suggested issues, order them by dependency, and split a slice into PRs small enough to review. Close a phase only when its exit criteria are observable.

### Phase 9: secure and correct the execution contract (highest priority)

**Issue candidates:** “Bind run creation to stored environment/scenario policy”; “Use approved script and per-run secret references”; “Make run terminalization atomic”; “Retain and secure partial artifacts”; “Reconcile operational docs and CI reproducibility.”

1. Add repository queries that load project, enabled environment, and scenario version together and prove ownership. Normalize the target to an origin and check membership in that environment's allowlist and a server-level allowlist; avoid trusting a free-form request script path. Scenario versions are immutable. Hash actual script bytes or a pinned source revision as well as effective config, then persist both.
2. Introduce a validated `ExecutionSpec` created on the server from the persisted snapshot. The runner receives script identity, origin, workload, hard deadline, allowed routes/hosts, limited environment variables and secret references. Inject secrets into the child only at launch; document the trusted execution environment. Resolve the actual Canvassync route and auth contract with the application owner before treating the example script as production ready.
3. Replace unconditional status writes with transactional compare-and-set transitions. Model acquire, cancel request, process stop, report commit, timeout, stale ownership, restart, and error after acquisition. Define a small, explicit concurrency limit. Ensure failure in any post-acquisition step leaves a durable terminal state, with a bounded reason; verify races and restart recovery with PostgreSQL tests.
4. Store artifact metadata in its own table with kind, hash, size, partial flag, and retention timestamp. Make downloads resolve only persisted references; verify bytes/hash, prevent traversal/symlink escapes, and restrict access. Save partial artifact references on failed/canceled runs even without a report, if usable. Add fixture tests for truncation and tampering.
5. Reconcile bind/migration/deployment docs. Add committed Rust/npm lockfiles where appropriate; change CI to `npm ci`. Make health/readiness reflect configured dependencies. Fix misleading latest-run/baseline copy.

**Exit:** malformed IDs, cross-project scenarios, disabled environments, unapproved origins/scripts and workload overruns are rejected. All execution outcomes have a durable, correct terminal state; cancellation cannot turn into completion. Fake k6 receives the exact approved nonsecret parameters and secret injection is testable without logging secrets. CI is reproducible and green.

### Phase 10: usable configuration and run workflow

**Issues:** “Project/environment/scenario registry API”; “Private operator authentication”; “Dashboard onboarding, preflight and run controls”; “Run history pagination and filtering.”

1. Create versioned APIs for project and environment creation/update/disable, immutable scenario versions, and listing details. Choose a minimal config schema with validated target allowlists, workload maxima, permitted script IDs, routes, secret-reference names and retention policy. Support safe config import for Canvassync and Knowdex; no secrets in exported config. Define uniqueness and ownership constraints in SQL.
2. Implement basic single-owner authentication for any deployment accessible beyond localhost: protect writes, run execution/cancel, reports and artifact downloads, with session expiry, CSRF protection as appropriate, and audit events. Document local-only development and deployment configuration. Never accept unauthenticated destructive run controls on a publicly bound API.
3. Build a complete “New run” UI: select project/environment/version, view the exact effective policy, edit workload within caps, preflight, confirm, enqueue and execute, inspect progress, cancel. Use a server-managed execution flow that does not depend on a browser staying connected. Clarify queued versus executing semantics.
4. Add project/run scoping, pagination, filters, descriptive errors, keyboard accessible controls, and empty states. Show actual latest run separately from an explicitly saved baseline.

**Exit:** an operator can create a local example project and execute a deterministic fake-k6 run end to end using the UI, without SQL inserts, manual POSTs, arbitrary script paths, or exposure of secrets.

### Phase 11: correct results and time-series visualization

**Issues:** “Define metric/report semantics”; “Ingest bounded k6 time buckets”; “Build time-series charts and exports”; “Threshold configuration and failure semantics.”

1. Specify report v2: actual UTC start/end, measured elapsed time, scenario/script hash, release/commit label when supplied, target, effective workload, units, numerator/denominator for rates, explicit `null` for missing values, threshold expression/result, k6 version, warnings and artifact hashes. Keep v1 readable and test migrations/serialization.
2. Ingest raw k6 JSONL as a stream into per-run bounded buckets for request rate, error rate and latency. Preserve source artifacts. Validate types/units/timestamps, parse malformed lines safely, and avoid loading unbounded runs entirely into memory. Choose a histogram or other sufficient aggregation to make latency percentiles meaningful; document chart aggregation limits. Initially store derived buckets in PostgreSQL or a compact artifact; measure before adding another store.
3. Show charts with axes, units, no-data gaps, threshold bands and time range, plus summary p50/p90/p95/p99 and threshold outcomes. Accessible table/download fallback. Reconcile k6's nonzero threshold exit with a valid measured report: a threshold-failed run is evidence, not an infrastructure success and not an empty report.
4. Offer machine-readable JSON and a shareable human-readable report that includes limitations and provenance. Decide redaction and auth rules for exports. Verify real k6 fixture output as well as fake process output.

**Exit:** a local run with realistic sample JSONL produces accurate summaries, charts and exports. Missing samples are visibly missing; failing thresholds show the measured outcome; report v1 remains readable.

### Phase 12: baselines, comparison and regression rules

**Issues:** “Explicit baseline selection and history”; “Compatible run comparison API”; “Regression policy and comparison UI.”

1. Add one explicit baseline selection per project/environment/scenario version (or named immutable baseline revisions) with author/time and replacement history. Only completed runs with valid report data qualify; never infer baseline from latest run. Preserve previous baseline IDs.
2. Define compatibility: same project, environment and target identity, scenario/script version and workload shape for direct claims. Display differences and allow exploratory comparisons with warnings, but do not label incompatible runs a regression. Respect absent or sparse metrics.
3. Calculate absolute and percent changes for p50/p90/p95/p99, request throughput and error rate. Define percent change around zero and whether lower/higher is better; show raw values and definitions. Support configurable tolerance/threshold outcomes and explicitly separate threshold failure from statistically meaningful regression. Avoid false precision from short or single runs.
4. Build side-by-side and overlay views with baseline metadata and downloadable comparison JSON/human report. Test incompatible pairs, missing metrics, zero denominators and reversed direction.

**Exit:** two comparable local runs can be compared and exported without manual cleanup; the UI explains compatibility and clearly distinguishes measured regression, threshold failure and inconclusive evidence.

### Phase 13: Canvassync proof and Knowdex reuse

**Issues:** “Validate Canvassync read journey and test data”; “Execute reviewed Azure baseline”; “Verify second-run comparison”; “Onboard Knowdex by configuration”; “Deployment and operating guide.”

1. Validate Canvassync's actual sign-in, cookie/token handling, endpoints and room-read response against a dedicated staging or approved Azure target. Seed one isolated room and test account. Prove script refuses missing inputs, unexpected redirects, wrong origin and forbidden routes. Verify secret loading without writing secret values into artifacts.
2. Have the operator review exact origin, account, data, 1-VU proposed workload, 60-second duration, think time, hard stop, Azure capacity, rollback/stop criteria and execution window. Execute the first live run only after those concrete inputs are supplied and reviewed; retain report and artifacts securely. Observe errors and latency; adjust scenario only through a new version.
3. Execute a second reviewed run under comparable conditions, save baseline and comparison, explain variability and limits. Validate report provenance and that raw output is safe to retain. Do not present one synthetic run as a general user experience claim.
4. Add Knowdex with a new project/environment/scenario and secret refs using public product configuration flows. Any target-specific script lives as versioned scenario content; no hardcoded Knowdex logic in core Rust. Document onboarding for a third project and use Metrics itself as a later test target.
5. Deploy with persistent PostgreSQL/artifact volume, least privilege, backup/restore, operator auth, redacted logs, readiness, resource caps, recovery, retention and upgrade path. Exercise restore and process restart in a nonproduction environment.

**Exit:** a real Canvassync baseline and comparison are accessible and reproducible; Knowdex is registered through configuration and can run an approved scenario; the deployment survives restart and restore with report/artifact integrity.

### Phase 14: later observability increments

After the performance workflow is proven, scope real-user and infrastructure telemetry as distinct sources and correlate on project/environment/time/trace IDs. For AI observability, design a separate OpenTelemetry-compatible ingestion interface and events for provider/model, operation, latency, outcome, tokens when available, and estimated cost when available. Exclude prompt/completion bodies by default; require an explicit opt-in and retention policy for content capture. Pilot one provider and one application before expanding. Do not collapse synthetic HTTP percentiles and AI latencies into a single metric.

**Exit:** a separate proposal and small instrumented pilot with privacy, retention, correlation, data-volume and operating-cost evidence. This is outside the initial MVP.

## 6. Suggested issue and PR breakdown

Keep dependencies visible in the tracker:

```text
P9 policy binding ─┐
P9 runner inputs ──┼─> P10 configuration + run UI ─> P13 live Canvassync/Knowdex
P9 lifecycle ──────┤                    │
P9 artifacts ──────┘                    └─> P11 results/charts ─> P12 baseline/compare ─> P13
P9 auth threat model ─> P10 auth ────────────────────────────────────────────────> P13
```

Open separate issues for each Phase 9 risk, then Phase 10–12 feature slices. An issue should have: user story, current source behavior, API/schema/UI impact, safety and failure cases, concrete tests, docs changed, and definition of done. Use multiple commits per meaningful issue where it improves reviewability; do not create arbitrary commits just to hit a count. A normal PR should include tests, implementation and docs and link the issue. Sequence migrations before dependent API/UI work. Record the PR URL, merged SHA, CI result and next dependency in the issue/handoff after merging.

## 7. First actions for the next agent

1. Identify the connected private GitHub repository, current default branch, working tree/branch availability, open issues/PRs and latest CI result. Check whether this workspace snapshot corresponds to `main`; obtain a proper writable checkout if needed. Do not overwrite remote changes from a stale snapshot.
2. Read the PRD and design docs linked above; run the full local quality gate with PostgreSQL and fake k6. Record any missing prerequisites instead of treating a format check as proof of functionality.
3. Inspect the six risks in section 3 against the current remote code and create focused issues for confirmed gaps. Begin with Phase 9 policy binding, child input wiring and lifecycle terminalization; they block a trustworthy real test.
4. Make a narrow feature branch/PR for the first issue, lead with a failing regression test, implement the minimal correction, update contracts/docs, run CI, then continue through dependencies. Keep the user updated on issues, commits, tests, PRs and phase exit checks.
5. Before any Azure run, assemble the concrete target/workload/account/seeded-data review and evidence checklist. If an essential external input is unavailable, finish all independent local work and request only the missing input.

## 8. Definition of done for the overall goal

The project is ready for its first performance MVP when a protected operator, without hand-editing the database, can configure Canvassync and Knowdex, preflight and execute approved k6 scenarios, stop them safely, read truthful summaries and time series, export raw and human reports, save a baseline, compare a second compatible run, and reproduce the configuration that generated both. Local/CI verification is green, a reviewed Canvassync baseline actually exists, secrets remain private, artifacts survive restart, and docs match deployment behavior. AI observability begins as its own subsequent increment.
