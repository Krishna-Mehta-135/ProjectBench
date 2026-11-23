# W6 AI claim status

Issue #102 defines three experiments. All three claim entries are **measuring**. No AI verdict or usage number is published.

| Claim | Required observation | Current state |
| --- | --- | --- |
| `canvassync-ai-fallback-chain` | Local fake provider returns 429 for the primary model and a valid response for the second; record success and latency difference from primary success. | Worker fallback code and fake-provider controls exist. The existing fake server uses one global 429 rate, so a deterministic model-specific fault and full worker run are still needed. |
| `knowdex-ai-token-stream-rate` | Fixed-rate fake provider, concurrent client requests, client first-token time, delivered fake tokens per second, completion and error counts. | The fake server streams four fixed chunks. No concurrent client measurement exists. |
| `canvassync-ai-token-usage` | Roughly 20 approved real Gemini calls; record API-reported counts per request and aggregate without prompt or completion text. | No accepted API usage observations exist. Fake responses cannot supply real usage counts. |

The fake-provider experiments must not be presented as real Gemini latency or token usage. A client text chunk is not necessarily a provider token. Store only timings, counts, model identifier, workload, and nonsecret environment metadata. Run the experiments locally and ingest schema-v1 files before changing any claim to verified.

No Docker or Gemini call was made while preparing this status. Docker's API socket was unavailable; real API credentials were not inspected.
