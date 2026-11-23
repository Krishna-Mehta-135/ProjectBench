# Evidence publication checkpoint

Prepared 2026-11-24 for issue #122. This is a review of the proposed public evidence set, not a published result. The public `evidence/` directory is absent and the evidence site is not deployed.

## Proposed first publication set

| Claim ID | Current finding | Public wording boundary | Publication state |
| --- | --- | --- | --- |
| `canvassync-redis-cold-start-latency` | W4/W5 classify the 100-, 1,000-, and 5,000-shape comparisons as inconclusive. | Historical baseline was a direct PostgreSQL read on join. Do not say “DB polling” or “60% faster.” | Hold until the 180 accepted schema-v1 samples and computed verdict are restored, inspected, and approved. |
| `knowdex-coalesce-volume` | W3 found 80% fewer outbound update frames per receiver with five simultaneous typists (100 to 20). One typist and AI stream had 0% median reduction. The paste workload failed its generator CPU validity gate. | The 80% figure applies only to the five-typist local workload. Do not publish the broad 90% claim or a paste result. | Hold until accepted schema-v1 samples and the computed verdict are restored, inspected, and approved. |
| `projectbench-demo-health` | The bundled local demo can exercise an HTTP health behavior claim. | A demo observation is not performance evidence. | Hold until the demo runs and its schema-v1 results and verdict pass review. |

The first publication set may be smaller if any result fails review. A public index entry must have its corresponding reviewed claim JSON. Portfolio claims stay “Measuring” until their IDs appear in the published index.

## Evidence needed at the checkpoint

1. Restore the locally ignored W3 and W4 raw result files, or rerun the same workloads on local Docker Compose. The current checkout contains neither set. Do not reconstruct samples from summary tables.
2. Ingest each accepted result into ProjectBench and compare the computed verdict with the W3 and W4 writeups. Record the exact source commit, container image digests, repetitions, workload, hardware limits, generator CPU peak, and reproduce command.
3. Inspect the generated `evidence/index.json` and every claim JSON. The publish allowlist excludes source refs, verification prose, credentials, prompts, and completions; still review the actual output for secrets, personal paths, customer data, and unexpected identifiers.
4. Review the static evidence site at 360, 768, and 1440 px. Check claim wording, workload labels, verdict explanations, sample plots, keyboard use, and links from the portfolio. Verify that a missing evidence index leaves the portfolio at “Measuring.”
5. Present the exact JSON files, rendered claim pages, and intended public origin to the owner. Record the owner's checkpoint decision before deploying the evidence site or adding evidence files to the public branch.

## Current execution limits

- Docker's API socket is unavailable on this machine at the time of this review. Do not restart Docker Desktop or load production to work around it.
- The W3 and W4 raw files are intentionally ignored by Git and are absent from the inspected local checkouts. The published site cannot show their sample-level evidence yet.
- The CanvasSync staging read journey (issue #67) has no approved origin, account, seeded room, workload, stop criteria, or execution window. It is separate from the local claims and must not be run against production.
- W6 AI claims (issue #102) have no accepted results. Fake-provider tests can establish behavior, but must be labeled as fake-provider observations; real token accounting requires approved real calls and an API key.

## Owner checkpoint decision

Pending. The publication set above is a candidate, not an approval. The source results, exact JSON, and rendered pages must be attached to this checkpoint before the final publication decision.
