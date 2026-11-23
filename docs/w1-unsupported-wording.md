# W1 unsupported-wording report

This is the W1 source audit before the W3/W4 measurements. For current measured findings, see [W10 proposed wording](w10-proposed-wording.md), [W6 AI status](w6-ai-claim-status.md), and the [evidence publication checkpoint](evidence-publication-checkpoint.md). The historical counts and “unmeasured” statements below describe W1 only.

Status: evidence is not yet measured. No resume or application source was edited. The ledger contains 88 entries: 17 resume, 14 README, and 57 app-doc entries (54 CanvasSync-doc and 3 Knowdex-doc); 7 are verified from direct app source/workflow/configuration citations, 1 code-fact wording is revised, and 80 behavior/comparison/proprietary or documentation assertions remain unverified. No numeric comparison is marked verified. The original Knowdex resume wording claiming general little-endian uint16 framing is revised/incorrect; the precise source-supported wording is “Knowdex AI-update frames encode text length as a little-endian uint16.”

## Findings

- `../Career-Ops/cv.md:36` says the Redis sync layer cut cold-start sync latency about 60% versus a DB-poll baseline. At `76a5fe0^`, an authorized `join_room` called `initializeRoomSync`; a cold room executed one Postgres `shape.findMany`, cached the result in the process, and returned `room_joined` (`apps/ws-backend/src/ws/roomSync.ts:16-39` and `apps/ws-backend/src/ws/messageHandler.ts:68-115`). There is no polling loop. C1 is unverified. Its baseline and candidate must be the same current build: reimplement the one-read/in-process-cache path behind a flag, then compare that flag-on path with the Redis flag-off path under identical workload, seed, limits, host, and repetitions. Do not benchmark `76a5fe0^` against the current build and do not use revised wording before measurement. Proposed owner wording after measurement: “Redis-backed room snapshots changed cold-join latency by {measured percent}% under {workload and limits}.”

- `../Career-Ops/cv.md:7` calls CanvasSync “CRDT-based” and says “at scale.” The current code has `mergeCanvasCrdtSnapshot`, which compares version metadata, applies incoming shapes, records deletions as tombstones, and returns one merged snapshot (`../Canvassync/packages/common/src/canvas-crdt.ts:111-165`), but the inspected usage is not a published convergence or scale measurement. Proposed owner wording: “versioned snapshot merge with tombstones; multi-node evidence pending.”

- `../Career-Ops/cv.md:45` says the 16 ms Knowdex buffer cut outbound WebSocket volume “about 90%.” The 16 ms timer and merge-before-broadcast path are source-verified (`../Knowdex/apps/ws-backend/src/coalesce-buffer.ts:9-18,119-177`), but the percentage is unmeasured and depends on burst shape. Proposed owner wording: “coalesces updates in a 16 ms window; local evidence pending.”

- `../Career-Ops/cv.md:37` says CanvasSync has a “token-bucket” rate limiter. The current HTTP middleware uses a Redis-backed fixed-window counter with defaults of 120 requests per 60000 ms and fails open on Redis errors (`../Canvassync/apps/http-backend/src/middlewares/rate-limit.middleware.ts:10-12,51-107`). The wording should name the fixed-window behavior unless another token-bucket path is identified.

- `../Career-Ops/cv.md:44` says Knowdex uses little-endian uint16 framing. The cited implementation only uses a two-byte little-endian length for the `ai-update` text field (`../Knowdex/packages/crdt/src/protocol.ts:62-77,145-163`); it does not establish general uint16 framing. The resume wording is revised/incorrect. Proposed precise wording: “Knowdex AI-update frames encode text length as a little-endian uint16.”

- The CRDT implementation details are retained on the genuine `canvassync-crdt-at-scale` resume claim; no duplicate resume-cited code-fact row or measured convergence result is claimed.

- The audit changed ten former verified rows to `unverified` because their verification relied on README/docs text or indirect references without an exact app source, workflow, or Compose citation directly proving the complete statement. The remaining seven verified rows cite direct implementation or deployment configuration lines. All numeric comparison claims remain unverified.

- `../Career-Ops/cv.md:38` says CI/CD deploys 3 Docker images. The deployment workflow directly names three application image builds and publishes them, then deploys the stack to the Azure app VM (`../Canvassync/.github/workflows/deploy.yml:55-61,76-86,94-95,200-203`). This is a configuration fact, not a deployment measurement; keep it as an exact topology statement, not a performance claim.

- AVAJet statements at `../Career-Ops/cv.md:25-28` remain `proprietary: true` and `status: unverified` in the ledger. They are not withdrawn; they must not be published as evidence or expanded with source details unless the owner actually withdraws them or supplies permitted verification.

## Source coverage note

The 45 new `source: docs` rows cover all eleven previously listed files: `architecture-canvas-engine.md`, `architecture-db-resilience.md`, `architecture-monorepo-strategy.md`, `architecture-overview.md`, `architecture-realtime-sync.md`, `architecture-security-model.md`, `codebase-walkthrough.md`, `core-concepts.md`, `developer-guide.md`, `persistence-layer.md`, and `ws-redis-sync-architecture.md`. Each row has an exact documentation source line, metric definition, status, and provenance note. No unrelated numeric values were grouped into one row.

Remaining gaps are explicit: all 45 newly covered assertions remain unverified; no runtime benchmark, fault-injection run, browser measurement, multi-node convergence result, or source-code corroboration was invented. The existing C1 comparison remains unmeasured and must compare the pre-Redis-compatible one-Postgres-read/in-process-cache path reimplemented behind a flag in the same current build against the Redis path under identical workload, seed, limits, host, and repetitions. Documentation targets such as 60+ FPS, <5 ms, 95% reduction, 80% CI reduction, and scale claims are not measurements.

## Fairness review for W1

The planned C1 and C2 comparisons are only fair if the same current build, local Docker images, data seed, client workload, host limits, repetitions, and measurement boundary are used for baseline and candidate. C1 must reimplement the pre-Redis single-Postgres-read plus in-process-cache behavior behind a flag; it must not compare an old commit with the current build. Other differences include Redis/RabbitMQ availability and warm versus cold cache state. A harness could measure itself if the client counts its own sends rather than receiver-observed frames, or if instrumentation changes the path; receiver-side frame counts and an independent convergence check are required.
