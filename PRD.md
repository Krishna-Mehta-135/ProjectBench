# Metrics — Product Requirements Document

**Status:** Initial draft  
**Date:** 2026-11-13  
**Working name:** Metrics  
**Repository folder:** ProjectBench

## 1. Summary

Metrics is a self-hostable, Grafana-inspired project observability and performance testing platform. It helps a developer run repeatable load tests against their own applications, inspect performance and reliability signals, and retain comparable reports as an evidence trail for each project.

The first targets are Canvassync and Knowdex, both deployed on Azure and currently without users. The first release should exercise one representative Canvassync user journey, establish a baseline, and make the same workflow reusable for Knowdex and future projects. Metrics itself is also intended to be stress-tested as new projects are added.

The implementation direction is Rust for the Metrics backend and k6 for traffic generation. Rust is the user's chosen language for building this project; k6 remains the load-testing engine rather than reimplementing a load generator.

## 2. Problem

Project performance checks are difficult to compare when test scripts, run parameters, results, and infrastructure details live in separate tools or are recreated by hand. A developer needs a repeatable way to answer:

- What was tested, against which deployment, and with what workload?
- How did latency, throughput, and failures change between runs?
- Did a release improve or regress the experience?
- Can the same test and report format be reused across projects?
- For AI features, what were model/provider latency, errors, token use, and cost?

Metrics should make those answers available from a consistent project-oriented interface and preserve the underlying run metadata so results are interpretable later.

## 3. Goals

1. Define projects, environments, and reusable test scenarios.
2. Run k6 scenarios against authorized target applications and capture complete run metadata.
3. Present a useful run overview with latency percentiles, throughput, error rate, thresholds, and time-series charts.
4. Compare runs for the same scenario and surface meaningful regressions.
5. Keep reports reproducible and exportable, with raw result data available for later analysis.
6. Support a Canvassync baseline first, then reuse the workflow for Knowdex and other projects.
7. Establish a safe foundation for AI observability without making AI telemetry a prerequisite for the first performance-testing release.

## 4. Non-goals for the initial release

- Replacing k6 with a custom load generator.
- Supporting arbitrary third-party targets without explicit project configuration and authorization.
- Full production incident management, paging, or an on-call system.
- A general-purpose log aggregation platform.
- Capturing prompt or completion content by default.
- Distributed test execution across a large fleet of workers.
- Multi-tenant SaaS, billing, or public user accounts.

## 5. Users and primary workflows

### Primary user

The project owner/developer who deploys and maintains Canvassync, Knowdex, and future applications.

### Workflow A: establish a baseline

1. Create a project and environment, such as Canvassync / Azure staging.
2. Define a scenario with a target journey, workload, duration, and safety limits.
3. Run the scenario through k6.
4. Inspect the summary, thresholds, and time series.
5. Save the run as the scenario baseline and export its report.

### Workflow B: compare a new run

1. Run the same scenario against the same environment after a change.
2. Compare it with the baseline or a selected previous run.
3. See absolute and percentage changes in p50/p95/p99 latency, request rate, error rate, and threshold status.
4. Keep the run associated with its configuration and optional release/commit metadata.

### Workflow C: add another project

Create Knowdex or another project, configure its environment and scenario, and use the same run/report workflow without changing the platform.

## 6. MVP scope

### Included

- Project and environment registry.
- Scenario definitions backed by versioned k6 scripts/configuration.
- Manual run start/stop for a single local or hosted runner.
- Safe workload controls: maximum virtual users, ramp-up, duration, and an explicit target allowlist per environment.
- k6 execution and ingestion of machine-readable results.
- Run status and metadata: project, environment, scenario version, start/end time, workload, target, and optional commit/release label.
- Summary metrics: request count, throughput, error rate, p50, p90, p95, and p99 latency; threshold pass/fail.
- Run detail charts and a comparison view for two runs of the same scenario.
- Persistent storage for configuration and run records, plus retained raw result artifacts.
- Downloadable JSON report and a human-readable report view.
- One Canvassync journey and baseline; configuration pattern ready to add Knowdex.
- Basic operator authentication appropriate for a private single-owner deployment.

### Deferred

- Scheduled runs and CI-triggered runs.
- Multi-runner/distributed k6 execution.
- Advanced alerting, dashboards builder, and custom query language.
- OpenTelemetry traces/logs/metrics ingestion beyond what is required for basic app instrumentation.
- AI provider auto-instrumentation and model-specific dashboards.
- Multi-user access control and hosted SaaS operations.

## 7. Functional requirements

### Projects and environments

- A project has a stable ID, name, description, and creation/update timestamps.
- An environment belongs to a project and stores a display name, base URL, and target allowlist.
- Secrets such as credentials are stored separately from scenario source and are masked in the UI and reports.
- Deleting or disabling an environment prevents new runs but does not remove historical run records.

### Scenarios and execution

- A scenario has a name, description, script/config version, and workload profile.
- Users can start a run, inspect its progress, and stop it.
- Each run snapshots the scenario version and effective configuration so later edits cannot rewrite history.
- The runner rejects targets outside the environment allowlist.
- The runner enforces configured maximum VUs and duration and displays the target and workload before execution.
- A failed or interrupted run is retained with its status and any available partial result data.

### Results and comparison

- The run overview shows status, target, scenario, workload, duration, and key performance statistics.
- Time-series charts show request rate, latency, and failures over the run.
- Thresholds are configurable per scenario and have explicit pass/fail outcomes.
- Comparison is limited to compatible runs of the same scenario; differences in environment or scenario version are disclosed.
- Report export includes metric definitions, workload settings, timestamps, target identity, threshold results, and relevant warnings.

### AI observability foundation

- The data model should permit AI spans/events to be linked to a project, environment, and trace/run without forcing AI fields into ordinary HTTP metrics.
- A future AI event should support provider/model, operation, duration, outcome/error category, token counts when available, and estimated cost when available.
- Prompt/response bodies are excluded by default. Any future content capture must be opt-in, clearly indicated, and protected by retention controls.
- AI observability should be added through a documented ingestion interface (prefer OpenTelemetry-compatible conventions where practical) rather than provider-specific dashboard assumptions.

## 8. Quality requirements

- Results must be traceable to the exact scenario/configuration and target environment used.
- The UI must distinguish a real zero from missing or unavailable measurements.
- The platform should report its own errors clearly and must not label a failed run as a performance pass.
- Access to run controls and secrets must be protected; target allowlists and workload caps are required safety controls.
- Core components should be deployable on Azure using a documented, repeatable configuration.
- The design should allow storage and execution components to scale independently later, while keeping the initial deployment simple.

## 9. Proposed technical direction

This is a starting architecture, subject to validation during implementation.

- **Backend/API:** Rust, using an async HTTP framework (Axum is the leading candidate).
- **Load generation:** k6 as a separate process/container, invoked with a controlled runner interface.
- **Frontend:** TypeScript web UI; keep it a separate client of the API.
- **Metadata and results:** PostgreSQL for projects, environments, scenarios, and run metadata. Start with k6 JSON output stored as run artifacts; evaluate a time-series store only when data volume or query needs justify it.
- **Deployment:** Docker-based local development and an Azure deployment path.
- **CI:** GitHub Actions for formatting, linting, build checks, and later automated scenario execution where safe.

The first version should favor a modular monolith and a single runner. Do not introduce a message broker or separate services until measured load or operational needs require them.

## 10. Core data concepts

- **Project:** an application being monitored/tested.
- **Environment:** a deployment of that application, with a base URL and safety policy.
- **Scenario:** a versioned k6 script and workload/threshold configuration.
- **Run:** one execution of a scenario against an environment, with immutable metadata and status.
- **Metric sample:** a timestamped observation emitted by a run or future instrumentation source.
- **Artifact:** raw output and generated reports associated with a run.
- **AI event (future):** model-operation observation linked to a project/environment and optionally a distributed trace.

## 11. Success measures

- A first Canvassync scenario can be configured and run end-to-end from Metrics.
- A second run can be compared with its baseline without manual data cleanup.
- Every displayed result can be traced to the exact target, workload, and scenario version.
- Knowdex can be added as a new project using configuration rather than platform code changes.
- The platform can identify threshold failures and show the metric that caused them.
- A future AI event can be represented without changing the core run/result model.

## 12. Open decisions

- Which Canvassync journey is the first representative load-test flow, and what credentials/test data does it require?
- Which environments and Azure URLs should be allowed for Canvassync and Knowdex?
- What request rate, VU cap, duration, and abort thresholds are safe for each target?
- Should the first release store artifacts on local disk or Azure Blob Storage?
- Which authentication method fits the initial private deployment?
- Which AI providers and frameworks matter first, once the performance MVP is established?

## 13. Delivery milestones

1. **Foundation:** repository structure, local development setup, API skeleton, database schema, and documented configuration.
2. **First run:** project/environment/scenario configuration and controlled k6 execution for one Canvassync journey.
3. **Results:** run history, summary metrics, charts, threshold results, and JSON export.
4. **Comparison:** baseline selection and regression comparison for compatible runs.
5. **Reuse:** add Knowdex through configuration and document the onboarding process for future projects.
6. **AI observability discovery:** choose an ingestion convention and a small set of provider/model fields, then scope a separate increment.

## 14. Risks and mitigations

- **A test overloads a live deployment:** require an allowlisted target, visible workload preview, hard caps, and an immediate stop control.
- **A result is misleading:** snapshot test configuration and show missing data, environment differences, and threshold definitions.
- **The platform becomes too broad:** keep the first milestone focused on one repeatable Canvassync scenario and comparison workflow.
- **AI data exposes sensitive content:** exclude prompt/response bodies by default and make any future capture explicit and retention-controlled.
- **Rust slows initial iteration:** keep the service as a modular monolith, use established libraries, and keep k6 responsible for load generation.

