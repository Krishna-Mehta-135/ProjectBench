# W10 proposed project and claim wording

This is proposed copy only. It does not edit the resume. The percentages from the original claim wording must not be reused as general statements.

## CanvasSync cold join

**Proposed wording:** “Implemented Redis-backed room synchronization in CanvasSync and measured cold joins against a direct PostgreSQL read baseline across rooms of 100, 1,000, and 5,000 shapes. The result ranges were inconclusive under the predeclared comparison rule, so I do not claim a latency reduction.”

**Reason:** W4 completed 30 valid joins per variant at each room size. W5 classified each size as inconclusive because the observed range widths did not separate the medians under the configured rule. The historical path was a direct database read on join, not database polling. Do not use “60% faster” or “DB-poll baseline.”

## Knowdex coalescing

**Proposed wording:** “In a five-typist benchmark, Knowdex’s 16 ms coalescing buffer reduced outbound update frames by 80%. The one-typist and AI-stream workloads showed no median reduction; the paste run was invalid because generator CPU exceeded the limit.”

**Reason:** The result is workload-specific. The broad “about 90% under keystroke-burst load” wording is unsupported. Keep the invalid paste result out of any percentage claim. See Knowdex PR #53 and follow-up issue #54.

## ProjectBench entry

“Built ProjectBench, a local Rust and PostgreSQL service that validates schema-v1 benchmark results against a claims ledger, computes deterministic verdicts, and writes allowlisted static evidence. It includes a read-only React evidence site and an isolated Docker demo for the ingest-to-verdict path.”

No performance number is attached to this entry. The demo's five local HTTP observations are a behavior example, not performance evidence.
