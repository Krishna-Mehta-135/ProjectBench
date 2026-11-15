# ProjectBench and portfolio: plan v2 (claims first, one week)

Prepared 2026-11-15 for implementation agents. This is the single source of truth. It replaces `docs/portfolio-metrics-handoff.md` and `docs/agent-handoff-plan.md`; use those only for local run commands.

## 0. Brief

Krishna Mehta put performance numbers on his resume without measuring them. In one week:

1. Measure those claims with reproducible local benchmarks.
2. Make ProjectBench the system that stores the results, computes a verdict per claim, and publishes a public evidence site.
3. Build a separate, simple portfolio site that links to that evidence.

Section 8 is the work for the week. Section 11 is the backlog and is out of scope until the week's work is public. Where an older document or the existing code disagrees with this plan, this plan wins.

### 0.1 Rules

1. Never invent, estimate, or round up a number. If a measurement contradicts the resume, report the measurement.
2. No load against production (`api.canvassync.tech`, `api.knowdex.me`). All benchmarks run on local Docker Compose.
3. Do not edit the resume (`Career-Ops/cv.md`). Propose wording to the owner.
4. Work on branches. Do not merge to `main`, push the app repos' unpushed commits, publish evidence, or change repository visibility without the owner's approval.
5. Preserve unrelated local changes: `Canvassync/design-prototypes/`, `Knowdex/.vscode/`, `Knowdex/Knowdex_Internship_Report.docx`.
6. Keep secrets out of code, logs, reports, and published files.
7. Do not stop processes you did not start. A Vite server (port 5173), the API (port 8080), and a PostgreSQL container were running at handoff.
8. No new control-plane features (leases, audit events, registry, preflight).
9. Do not add a feature because it reads well on a resume. Each addition needs a claim or a user need behind it.
10. Say "evidence" and "verified, reproducible". Do not call self-measured results an audit.
11. The product name is ProjectBench. Remove "Metrics" as a product name from UI and docs.
12. If an owner checkpoint is not answered, continue with unblocked work. Never publish without it.

### 0.2 Tooling: caveman and graphify

Caveman (plugin `caveman`):

- Keep caveman mode on for chat replies and status updates.
- Code, commits, PR descriptions, docs, published evidence, and all site copy are written in normal prose.
- Security warnings and confirmations for irreversible actions use full sentences.
- For delegation, prefer `cavecrew-investigator` (locate code), `cavecrew-builder` (one or two file edits), and `cavecrew-reviewer` (review a diff before a PR).

Graphify (skill `/graphify`):

- On day 1, run `/graphify` in ProjectBench, Canvassync, and Knowdex. Add `graphify-out/` to each `.gitignore`.
- Query the graph first for architecture questions. Use grep for exact strings.
- Rebuild after structural changes (end of days 4 and 6).
- Use it to draft architecture diagrams, then check them against code.
- Claims cite file and line from source, never from the graph.

### 0.3 Engineering workflow

Issues:

- Before writing code, create one GitHub issue per work package (W0 to W10) in the repo it touches. Bench work in Canvassync and Knowdex gets issues in those repos.
- Each issue carries the package's tasks as a checklist and its acceptance criteria copied from this plan. Group them under a milestone named "Week 1".
- Create one issue per backlog item (section 11) with the label `backlog`. Do not start them.
- One branch per issue, named after it (for example `w3-knowdex-coalesce`). One pull request per issue, with "Closes #n".
- A bug or question found along the way becomes its own issue. Do not fix unrelated things inside a package's pull request.
- If a package turns out larger than a day, split it into sub-issues and say so in the daily report.

Test-driven development:

- For logic, write the failing test first, then the code. The commit history shows the test before or with the implementation. This applies to: verdict math, each outcome rule, wording templates and rounding, result schema validation, the publish allowlist, claims loading, and the harness helper (median, range, interleaving, warm-up discard).
- Every bug fix starts with a test that reproduces it.
- Each harness has a self-check that runs before measurement: with coalescing off, frames received equals updates sent; all clients converge; a known input produces a known count. A harness that fails its self-check produces no result file.
- Test-first does not apply to layout, Compose files, or documentation. Check those by running them: browser at 360 and 1440 px, `make bench-up`, `make demo`.
- Never weaken, skip, or delete a test to make a build pass. If a test is wrong, say why in the pull request.
- Tests run without network access and without real Gemini calls.

Code quality:

- Rust: `cargo fmt --check` and `cargo clippy -- -D warnings` pass. No `unwrap` or `expect` outside tests. Typed errors. No module over about 400 lines after the `lib.rs` split.
- TypeScript: strict mode, no `any`. Add ESLint and Prettier to the web app and the portfolio and run them in CI. No component over about 200 lines.
- Harness code is held to the same standard as application code. A wrong harness produces a wrong number.
- No dead code, no commented-out code, no TODO without an issue number.
- Names say what a thing is. Comments explain why, not what.
- No new dependency without a one-line reason in the pull request.

Pull requests:

- One concern each, small enough to review in one sitting.
- Description states what changed, why, how it was tested, and what was not tested. UI changes include screenshots.
- CI must pass: format, lint, tests, build.
- Before asking the owner, run a review pass on the diff (`cavecrew-reviewer` or a stronger model) and resolve its findings.
- For W1, W3, W4, and the verdict rules in W5, the review must answer in writing: is the comparison fair, what else differs between the variants, and could the harness be measuring itself.

Sessions:

- One work package per session. Load this plan's sections 0 and the package itself, not the whole backlog.
- End each session with a short report: done, not done, blocked, needs owner review, and links to issues and pull requests.

### 0.4 Subagents

Roles. Create each as a subagent definition in the tool being used. In Claude Code that is one markdown file per role in `.claude/agents/` with `name`, `description`, `tools`, and `model` in the front matter and the role instructions as the body. In another tool, use its own agent definition format, or open a separate session with the role instructions at the top.

| Role | Responsibility | Model | Edits files |
| --- | --- | --- | --- |
| Lead (main session) | Chooses the package, creates the issue, delegates, opens pull requests, writes the daily report | any | yes |
| Builder | Implements one issue, test first | small model is acceptable | yes |
| Experiment designer | W1 claims ledger, A/B design in W3 and W4, verdict rules in W5b | strongest available | yes |
| Reviewer | Reviews every diff. For W1, W3, W4, and W5b answers the fairness questions in 0.3 in writing | strongest available | no |
| Investigator | Locates code and answers where and what questions | small model is acceptable | no |
| UI checker | Runs the site, captures 360 and 1440 px screenshots, lists layout and accessibility problems | small model is acceptable | no |

The existing `cavecrew-investigator`, `cavecrew-builder`, and `cavecrew-reviewer` cover the investigator role, small edits, and routine diff review.

Rules:

- The builder and the reviewer of a change are never the same agent.
- Run agents in parallel only when they work in different repos. Two agents in one repo each get their own git worktree.
- At most three agents at once.
- A subagent receives sections 0.1 and 0.3 of this plan and its one issue. Nothing else.
- Subagents do not merge, push to `main`, publish evidence, change repository settings, or touch the resume. Only the lead opens pull requests.
- Subagents do not start other subagents.
- Each subagent returns a short report: done, not done, files touched, tests run with their output. The lead verifies the report against the diff before relying on it.
- If only a small model is available for the week, the experiment designer and reviewer roles still go to a stronger model for W1, W3, W4, and W5b. These are a handful of sessions.

## 1. Findings

### 1.1 Resume claims

| ID | Claim on resume | Kind | Evidence today |
| --- | --- | --- | --- |
| C1 | CanvasSync: Redis sync layer cut cold-start sync latency about 60% versus a DB-poll baseline | Comparative, WebSocket timing | None |
| C2 | Knowdex: 16 ms coalesce buffer cut outbound WebSocket volume about 90% under keystroke-burst load | Comparative, frame count | None |
| C3 | CanvasSync: circuit breaker (5 failures / 60 s), idempotency store (10 min), token bucket, fails fast with 429/503 | Behavior | Code exists, untested under fault |
| C4 | Knowdex 10 message types, uint16 framing; CanvasSync 3 Docker images | Code fact | Readable from source |
| C5 | AI: 2-model fallback chain, token streaming, RAG pipeline | Behavior | Code exists, untested |
| C6 | AVAJet: 70+ types, 36 entity types, 6 concurrent queries, O(n) to O(1) | Proprietary | Cannot be published |

ProjectBench today runs one thing: a k6 HTTP read scenario at 1 virtual user against production. That measures none of these.

### 1.2 Facts

- Knowdex `apps/ws-backend/src/coalesce-buffer.ts:9` hardcodes `COALESCE_INTERVAL_MS = 16`. Coalescing arrived in commit `341b986`.
- CanvasSync Redis sync arrived in commit `76a5fe0`. A search of the current tree finds no DB-poll path and no mention of cold-start.
- CanvasSync has in-process counters with latency buckets in `apps/ws-backend/src/ws/metrics.ts`. They are not served over HTTP.
- CanvasSync sync messages are snapshots with room-version counters. The resume calls this a CRDT layer. Unverified.
- Neither app has a benchmark harness.
- Local `main` is ahead of GitHub by 40 commits (CanvasSync) and 28 (Knowdex).
- The ProjectBench working folder has no `.git`. GitHub has PR #97 merged. `../ProjectBench-handoff` is an older clone with uncommitted edits to `lib.rs` and `storage.rs`. The GitHub repository is private.
- The local API answers `GET /api/v1/projects` with no token and listens on `*:8080`. The runs list is empty.
- `apps/api/src/lib.rs` is about 1,950 lines. `apps/web/src/main.tsx` is one 221-line component.
- Laptop: Apple M5, 10 cores, 24 GB RAM. Docker has 10 CPUs and about 8 GB. About 44 GB of disk is free.
- Production: Azure VMs with 2 vCPU and 1 GB RAM, shared by all containers on the VM. Containers are capped at 384 MB.
- Both apps call Gemini.

### 1.3 Known risks to the claims

- C2: a 16 ms window merges only updates that arrive within 16 ms of each other. One person typing 10 characters per second sends one update per 100 ms, so almost nothing merges. A 90% reduction needs about ten updates per window (several typists, a paste, an AI token stream). Expect different results per workload.
- C1: the baseline may never have been a DB poll. The claim may need to become an absolute statement.
- "CRDT" for CanvasSync and "at scale" in the summary may not match the code or the usage.

## 2. Decisions

| Topic | Decision |
| --- | --- |
| Portfolio and ProjectBench | Separate repos. Owner's decision. |
| Portfolio | Astro, static, free hosting. Answers who the owner is. |
| ProjectBench public site | Static build of the web app, free hosting. Answers what was measured. |
| ProjectBench API, PostgreSQL, runner | Laptop only. Never public. |
| Evidence transport | ProjectBench publishes JSON into `evidence/` in its repo. The public site serves it at `/evidence/...`. The portfolio fetches `/evidence/index.json` from that site at build time. |
| Test bed | Local Docker Compose with production-like limits. |
| Harnesses | Node scripts inside each app repo, reusing real client code. |
| AI | Handled as ordinary claims about the apps' AI features. No separate AI observability subsystem. |
| k6 | Kept as is. Not used this week. |

## 3. How it works

```
claims.yaml (ProjectBench)      every claim, its source, metric, experiment
        |
bench harness (app repos)       Docker Compose with limits, Node clients,
        |                       baseline and candidate, N repetitions
        v                       writes result files (schema v1)
ProjectBench API + Postgres     ingests results, computes verdicts   [laptop]
        |
publish                         writes allowlisted evidence/*.json   [git]
        |
ProjectBench public site        landing, claims, claim pages, method [free static host]
        ^
portfolio                       fetches evidence index at build,     [free static host]
                                links each claim to its page
```

Visitor: portfolio, project, claim row, ProjectBench claim page with value, workload, hardware, commit, and reproduce command.

Owner: change app code, run `make bench-<claim>`, results are ingested, verdict updates, review, publish.

Future project: add a `bench/` folder that writes schema v1 and an entry in `claims.yaml`. No ProjectBench code change.

## 4. Positioning

Pitch: ProjectBench turns an engineering claim into a reproducible experiment with a verdict and a public evidence page.

Scope of "claim": any performance or behavior statement about software, wherever it is written. The resume is one source among several: README files, pull request descriptions, release notes, design documents, blog posts. The product is about claims on projects. The resume is the origin story, told in the README's background section and in interviews. Site copy, docs, and UI say "claims", never "resume claims".

This is a framing and data-model choice. It adds no integrations this week: no pull request bot, no badge service, no scanning of other repositories.

How it differs. Verify each row against the tool's current documentation before it goes into the public README.

| Tool | What it does | What ProjectBench adds |
| --- | --- | --- |
| Grafana with Prometheus | Monitors running systems | Experiments, A/B verdicts, claims |
| k6 Cloud | Runs and charts load tests | Claim model, custom harnesses beyond HTTP, local and free |
| Bencher, CodSpeed, github-action-benchmark | Track function-level benchmarks per commit | Whole-system A/B with workloads and resource limits |

The distinct part is the chain: claim, experiment, verdict, published evidence. Charts and run history are expected and are never the pitch. Do not call it a Grafana clone.

What makes it strong:

1. It solved a real problem for the owner and changed his resume.
2. Measurement rigor (section 5.2).
3. Real engineering in the Rust service.
4. A live site and a one-command reproduction.

Audience: recruiters and interviewers first. Other developers with unmeasured claims second, served by the quickstart and the add-a-project guide. Wide adoption is not a goal.

The origin story (unmeasured numbers, then measured) is told in the README and in interviews. Whether the public site shows original resume wording beside revised wording is the owner's decision (section 13). Default: it does not.

## 5. Hardware and cost

Required spend: zero.

### 5.1 Why the laptop is enough

- The laptop has 10 cores and 24 GB. Production is 2 vCPU and 1 GB.
- The system under test runs with production-like limits: all app containers pinned to the same 2 cores, each capped at 384 MB. The load generator uses other cores.
- C1 and C2 are ratios between two variants on one machine. Ratios transfer across hardware far better than absolute values.
- One Node process holds thousands of WebSocket connections.

### 5.2 Rules for valid results

1. Pin the system under test and the load generator to different cores (`cpuset`).
2. Record load generator CPU. Above 70%, the run is invalid.
3. Warm up, then measure. Discard the warm-up.
4. At least five repetitions per variant per workload. Report median, minimum, and maximum.
5. Interleave variants (A, B, A, B).
6. Baseline and candidate differ by one switch in the same build wherever possible.
7. Every result records CPU model, container limits, OS, Docker version, commit SHA, and image digests.
8. Every published result states its limits: Docker on macOS runs in a VM, the laptop is ARM, client and server share a host.
9. Delete raw artifacts after results are ingested. Disk is limited.

Owner actions before day 2: raise Docker memory to 12 GB, plug in power, close heavy apps during runs.

### 5.3 Gemini cost

- Benchmarks never call Gemini. Run a small fake Gemini HTTP server in the bench Compose file (configurable delay, token rate, 429 rate) and point the app at it by base URL. If the SDK cannot be redirected, add a provider switch by environment variable.
- Real Gemini calls: about 20, within the free tier, for token counts only.

### 5.4 Hosting

Free static hosting for the portfolio and the ProjectBench public site. Nothing else is hosted.

## 6. Scaling

- More projects: a `bench/` folder and a claims entry.
- More data: results are small JSON. PostgreSQL is enough.
- More load: one laptop generator is enough for systems limited to 2 vCPU.
- Beyond the laptop: the existing run lease allows a separate worker later. Do not build it.

## 7. Do not do

- Load against production. Soak tests.
- Distributed runners, brokers, time-series database, blob storage.
- Multi-user accounts, dashboard builder, query language.
- A separate AI observability subsystem, trace ingestion, chat over metrics.
- Storing or publishing prompt or completion text.
- Redesign of the operator pages.
- CSS-drawn fake screenshots, or any status label not backed by data.
- Any number on a site or resume without a published result.
- Paid hosting.

## 8. The week

Work packages run in parallel where they touch different repos. Each has acceptance criteria. The owner checkpoint closes each day.

| Day | Packages | Owner checkpoint |
| --- | --- | --- |
| 1 | W0 repo, W1 claims ledger, W2 schema and bench foundations | Soften the two resume bullets. Approve claim definitions. Raise Docker memory. |
| 2 | W3 Knowdex experiment. In parallel: W5a ingest | Review Knowdex results |
| 3 | W4 CanvasSync experiment. In parallel: W5b verdicts and publish | Review CanvasSync results |
| 4 | W6 AI claims. W5c split `lib.rs`. Import all results | Approve what gets published |
| 5 | W7 public site, deployed | Review at phone and desktop width |
| 6 | W8 quickstart and docs. W9 portfolio | Review portfolio copy |
| 7 | W10 code tour, resume wording, secrets scan, review fixes | Read code tour. Update resume. Make repo public. |

If the week runs short, drop in this order: W6 AI claims, portfolio polish, docs beyond README and quickstart, code tour depth. Never drop W3, W4, or owner review. If an experiment cannot be made fair in its day, mark the claim "measuring", propose softened wording, and move on.

### W0. Repo (day 1)

- Clone ProjectBench fresh from GitHub. Copy the working folder over it on branch `plan-v2-base`, excluding `target/`, `node_modules/`, and `dist/`. Review the diff, commit, open a PR.
- Compare the uncommitted edits in `../ProjectBench-handoff` with `main` and report anything missing. Do not delete that folder.
- Bind the API to `127.0.0.1` by default. Refuse another address unless `METRICS_OPERATOR_TOKEN` is set.
- Run `/graphify` in the three repos.
- Create the issues, milestone, and backlog issues described in 0.3.
- Add ESLint and Prettier to the web app and wire them into CI.

Acceptance: one git checkout is the source of truth, CI is green including lint, and every work package has an issue.

### W1. Claims ledger (day 1)

- Create `claims/claims.yaml`. One entry per number or technical assertion, from every source: the resume, both app READMEs, and the docs folders of both apps (for example the Knowdex README statement that bursts of edits are coalesced before persisting).

```yaml
- id: knowdex-coalesce-volume
  project: knowdex
  source: resume           # resume | readme | pull_request | release_notes | docs
  source_ref: "Career-Ops/cv.md"
  statement: "cut outbound WebSocket volume about 90% under keystroke-burst load"
  kind: comparative        # comparative | behavior | code_fact | proprietary
  metric: outbound_frames_per_receiver
  direction: lower_is_better
  baseline: coalesce off (interval 0)
  candidate: coalesce 16 ms
  workloads: [one_typist, five_typists, paste_burst, ai_stream]
  repetitions: 5
  status: unverified       # unverified | measuring | verified | revised | withdrawn
```

- Code facts: record file and line, mark verified or wrong.
- Read CanvasSync at `76a5fe0^` and write down what the join and sync path did before Redis. State whether "DB-poll baseline" describes it.
- Check "CRDT" for CanvasSync and "at scale" against the code and usage.
- Mark AVAJet items proprietary.

Acceptance: every claim has an entry with a metric definition. A short report lists wording the code does not support.

### W2. Schema and bench foundations (day 1)

- Result schema v1: `docs/result-schema.md` plus a JSON Schema file.

```json
{
  "schema_version": "1",
  "claim_id": "knowdex-coalesce-volume",
  "project": "knowdex",
  "variant": "candidate",
  "workload": "five_typists",
  "repetition": 3,
  "started_at": "2026-11-17T10:00:00Z",
  "duration_seconds": 60,
  "metrics": [
    { "name": "outbound_frames_per_receiver", "unit": "frames", "value": 0 },
    { "name": "delivery_latency_p99", "unit": "ms", "value": 0 }
  ],
  "environment": {
    "commit": "", "images": {}, "host_cpu": "", "sut_cpuset": "0-1",
    "sut_memory_mb": 384, "generator_cpu_peak_percent": 0, "docker": "", "os": ""
  },
  "limitations": []
}
```

- In each app repo: `docker-compose.bench.yml` (limits, core pinning, seeded data, fake Gemini server) and Makefile targets `bench-up`, `bench-down`, `bench-<claim>`.
- A small helper copied into each `bench/`: repetitions, interleaving, warm-up, generator CPU sampling, result writing.

Largest unknown of the week: whether each app starts fully on local Docker. CanvasSync production uses managed PostgreSQL and has no local database service in its Compose files, and both apps need environment files. Do this first on day 1. If an app cannot be started locally by the end of day 1, tell the owner immediately; its experiment day is at risk.

Acceptance: `make bench-up` starts each app with seeded data and limits. A trivial harness writes a schema-valid result.

### W3. Knowdex coalesce experiment, C2 (day 2)

- Read the interval from an environment variable. Zero disables coalescing. Default stays 16.
- Count outbound frames and bytes per connection. Expose on a bench-only endpoint.
- Harness: one document, 10 receivers, real Yjs clients using the shared `ProtocolCodec`.
- Workloads: one typist at 5 to 10 characters per second, five typists, a 2,000-character paste, a simulated AI stream at 50 tokens per second.
- Metrics: frames and bytes per receiver, delivery latency p50 and p99.
- Check that all clients converge to the same document after each run.

Acceptance: a table per workload with reduction and latency cost. Proposed resume wording naming the workload.

### W4. CanvasSync cold-start experiment, C1 (day 3)

- Metric: time from WebSocket open to the joining client holding full room state. Room sizes 100, 1,000, and 5,000 shapes.
- Baseline: reimplement the pre-Redis join path behind a flag in the current code, using `76a5fe0^` as the reference for what it did. Do not benchmark the old commit against the current one; 245 commits of unrelated change separate them.
- Define "cold" precisely (for example: no room state in server memory, Redis warm or flushed) and apply it to both variants.
- 30 cold joins per variant per room size.
- Serve the existing counters in `ws/metrics.ts` on `/metrics`.

Acceptance: median and range per variant and room size. If no fair baseline exists, propose an absolute statement (cold join of a 1,000-shape room in X ms under the stated limits).

### W5. ProjectBench: ingest, verdicts, publish (days 2 to 4)

- W5a: migration and endpoints for claims and results. `POST /api/v1/results` accepts schema v1. A small CLI or `make ingest` posts a folder of result files. Claims load from `claims.yaml`.
- W5b: verdict for comparative claims (median candidate versus median baseline, relative change, range, minimum repetition count, same environment across variants). Verdict for behavior claims (pass or fail with observed values). Publish action writes `evidence/<project>/<claim>.json` and `evidence/index.json` with allowlisted fields only.
- W5c: split `lib.rs` into modules with no behavior change.

Verdict outcomes and next steps (part of W5b). Every verdict carries a next step and, where it applies, a suggested wording. Both are produced by fixed rules and templates from stored results. No model is involved.

| Outcome | Rule | Next step shown |
| --- | --- | --- |
| verified | Measured change meets the claim within a stated tolerance in every listed workload | Publish. Keep as the baseline for reruns. |
| revised | Change is in the claimed direction but the size differs | Suggested wording with the measured value and workload |
| conditional | Holds in some workloads and not in others | Suggested wording naming the workloads where it holds |
| not_supported | No change, or change in the wrong direction | Remove the number and keep the mechanism, or change the code and rerun |
| inconclusive | Too few repetitions, range wider than the difference, generator CPU above 70%, or environments differ between variants | The specific reason and what to rerun |

- Suggested wording is a verified statement usable in any source (resume, README, pull request). It is a template filled with measured values, for example "cut outbound WebSocket frames {percent}% with {workload} ({candidate})". Round down, never up.
- Reason hints come from simple checks on the data, for example mean gap between updates compared with the coalesce window.
- The tolerance for "verified" is set per claim in `claims.yaml` and shown on the claim page.
- The claim page shows outcome, next step, and suggested wording in the operator view. The public page shows outcome and measured values only.
- Rerunning a claim after a code change adds a new result set. The claim page shows the history so a regression is visible.

Acceptance: C1 and C2 show verdicts from real results, each with a next step. Published JSON contains no secrets, private IDs, or host paths. Tests cover verdict math, each outcome rule, wording templates, and the publish allowlist.

### W6. AI claims, C5 (day 4, first to drop)

Handled through the same pipeline.

| Claim | Kind | Test |
| --- | --- | --- |
| Two-model fallback survives rate limits | behavior | Fake server returns 429 for the first model. Verify the second answers. Record added latency. |
| Token streaming under concurrent requests | behavior | Fake server streams at a fixed rate. Measure time to first token and tokens per second at the client. |
| Tokens per request | measured fact | About 20 real Gemini calls. Record token counts from the API response. |

Results from the fake server are labelled as such. No prompt or completion text is stored.

Acceptance: each claim has a verdict or is marked "measuring".

### W7. ProjectBench public site (day 5)

Stack: the existing Vite, React, TypeScript app. Add React Router, uPlot for charts, and Tailwind. Configure the static host to serve `index.html` for all routes.

Public routes:

- `/` Landing: one sentence on what ProjectBench is, "Built by Krishna Mehta" with portfolio link, scoreboard (claims verified, revised, measuring), project cards with headline result, how it works in three steps, the quickstart command, GitHub link.
- `/claims` List: claim, measured value, workload, verdict, date.
- `/claims/:id` Claim page: statement, metric definition, dot plot of every repetition with medians for both variants, result per workload, hardware and limits, commit, limitations, reproduce command with a copy button.
- `/projects/:slug` Project page: what it is, live and source links, its claims.
- `/method` Test bed, rules from 5.2, what the numbers do not show.

Rules:

- Every page has a header with project name, owner name, and portfolio link, and a footer with GitHub and contact.
- The public build reads only `evidence/*.json`. It contains no write control.
- With nothing published, the scoreboard reads zero and says measurement is in progress.
- Operator pages (the existing Lab plus a results and publish view) move under `/app` and exist only in the local build. They need to work, not look good.
- Open Graph image and metadata.

Visual rules:

- Design tokens for colour, spacing, and type in one file. Dark and light themes.
- One sans family and one monospace with tabular figures for numbers. One accent colour. Status colours for status only.
- Charts have axis units, hover readout, and a table fallback.
- Loading, empty, and error states. Missing values show a dash.
- Visible keyboard focus, AA contrast, reduced motion, usable at 360 px.

Acceptance: deployed URL loads with no API running, deep links work, checked at 360 and 1440 px with real published data, screenshots in the PR.

### W8. Quickstart and docs (day 6)

- `make demo`: starts PostgreSQL, the API, the web app, and a small bundled sample app with one sample claim and harness, runs the experiment, and shows the verdict. Docker only. No dependency on CanvasSync or Knowdex.
- `.env.example` with safe defaults.
- README in this order: pitch, owner and portfolio link, screenshot, live site link, what it is and is not, quickstart, how-it-works diagram, docs links.
- Docs: `quickstart.md`, `concepts.md`, `add-a-project.md`, `write-a-harness.md`, `result-schema.md`, `methodology.md`, `architecture.md`.
- Move outdated documents to `docs/archive/`.
- CI job runs `make demo` from a clean clone and checks for a verdict.

Acceptance: a clean checkout with only Docker reaches a visible verdict with one command. `add-a-project.md` can be followed without reading source.

### W9. Portfolio (day 6)

New repo. Astro, static, Tailwind, MDX.

- Home: name, one-line role from the resume summary, location, links (GitHub, LinkedIn, email, resume PDF). Three project cards with real screenshots, a one-sentence description, stack, and one headline measured result or "Measuring". Experience (AVAJet). Contact.
- Project page for CanvasSync, Knowdex, and ProjectBench: problem, screen capture, architecture diagram, two or three hard problems, a claims table whose rows link to the ProjectBench claim pages, live and source links.

Rules:

- Screenshots come from the running apps.
- A claim without published evidence shows "Measuring" and no number.
- The portfolio does not duplicate claim detail or method pages.
- No phone number unless the owner says so.
- Two type families at most. Hero heading does not clip at 360 px. Open Graph metadata.

Acceptance: deployed preview, builds from the evidence index, checked at 360, 768, and 1440 px, keyboard navigable.

### W10. Handover (day 7)

- `docs/code-tour.md`: each Rust module, what it does, why it is built that way, and likely interview questions with answers grounded in the code.
- Proposed resume wording for every measured claim and a ProjectBench entry that describes what exists.
- Secrets scan across the full git history of ProjectBench, a review of committed configuration, and an MIT license. Report to the owner.
- Fixes from the owner's reviews.

Acceptance: the owner can explain every module, every resume number maps to a claims entry, and the scan report is delivered.

## 9. Definition of done for the week

- Every number on the resume is verified, a code fact with a source reference, softened, or removed.
- A visitor can go from the portfolio to a claim page and see value, workload, hardware, commit, and reproduce command.
- The owner can run one command per claim, ingest, see the verdict, and publish.
- A stranger can run `make demo`.
- No published number comes from production or from a fixture.
- Recurring cost is zero.

## 10. Future projects

Adding a project means: create `bench/` in its repo following `add-a-project.md`, add entries to `claims.yaml`, run, ingest, publish, and add one MDX file to the portfolio. Any language works if it writes schema v1.

## 11. Backlog (after the week, in this order)

1. Bottleneck story: stress the app, capture a CPU profile, fix the top finding, publish before and after.
2. Capacity claim per app: highest concurrent WebSocket clients and request rate that hold a stated p95 under the limits. Use arrival-rate load.
3. Resilience evidence for C3: fault injection on PostgreSQL, breaker opening, 429 under burst, idempotency.
4. CI regression gate in the app repos on deterministic metrics. Extend it to pull request claims: a claim declared in the pull request is rerun and its verdict posted as a comment. Also a small "verified claim" badge for README files that links to the claim page.
5. `command` runner so ProjectBench launches a harness itself.
6. Per-bucket latency histograms for k6 runs.
7. `projectbench init` and `validate` commands.
8. AI assistance through the Claude API, in two places. First, draft `claims.yaml` entries and experiments from pasted resume text for the owner to edit. Second, on a claim that is not supported or inconclusive, explain the likely cause from the results and the relevant code and suggest a code or test change. In both, a model may explain and advise. It may never produce, estimate, or reword a number or verdict.
9. Optional: the Rust API on the existing Azure VM in read-only mode. Measure memory first.
10. Operator page redesign, telemetry scraping, Lighthouse evidence.

## 12. Estimates without the deadline

For reference only: about 15 working days for a single person for sections 8 and 9, and 15 to 20 more for the backlog.

## 13. Questions for the owner

Defaults apply if unanswered.

1. Push the unpushed commits in CanvasSync (40) and Knowdex (28)? Default: not pushed.
2. Portfolio repo name and domain? Default: repo `portfolio`, free preview URL.
3. Public contact details? Default: email, GitHub, LinkedIn. No phone.
4. Soften the two percentages on the resume now? Recommended. Owner's call.
5. Show original resume wording beside revised wording on the public site? Default: no.
6. Make the ProjectBench repository public after the scan? Default: owner switches it on day 7.
7. Keep the Azure deployments running for live links? Default: yes, untouched.
