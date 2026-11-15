# ProjectBench result schema v1

ProjectBench result files are JSON evidence produced by a local benchmark. A
result is valid only when it conforms to
[`result-schema.schema.json`](result-schema.schema.json). The schema is strict:
unknown fields are rejected at the result, metric, and environment levels.

Results describe one claim, one variant, one workload, and one repetition. The
`started_at` timestamp is RFC 3339 date-time text and must include a timezone.
`duration_seconds` and metric `value` are numeric measurements; values are not
rounded or estimated by the schema. `repetition` starts at 1.

## Required fields

| Field | Meaning |
| --- | --- |
| `schema_version` | The string `"1"`. |
| `claim_id` | Stable identifier of the claim being measured. |
| `project` | Project whose local benchmark was run. |
| `variant` | Compared implementation variant, such as `baseline` or `candidate`. |
| `workload` | Named workload definition. |
| `repetition` | One-based repetition number. |
| `started_at` | UTC/RFC 3339 start timestamp. |
| `duration_seconds` | Non-negative measured run duration. |
| `metrics` | At least one metric measurement. |
| `environment` | Reproduction details for the local run. |
| `limitations` | Statements that qualify interpretation of this result; use `[]` when there are none. |

Each metric has exactly `name`, `unit`, and numeric `value`. Metric names and
units are defined by the claim or harness; the schema does not reinterpret
them.

The environment has exactly these fields: `commit`, `images`, `host_cpu`,
`sut_cpuset`, `sut_memory_mb`, `generator_cpu_peak_percent`, `docker`, and
`os`. `commit`, `host_cpu`, `docker`, and `os` are non-empty strings. `images`
maps at least one image name to a non-empty image digest; digests use the form
`sha256:` followed by 64 lowercase hexadecimal characters.
The generator CPU peak is a percentage from 0 through 100. The system-under-
test memory limit is in megabytes and must be positive.

Results must contain evidence from local Docker Compose runs only. Do not put
credentials, cookies, prompts, completions, customer data, host paths, or
other secrets in result files. A benchmark that cannot record the required
environment or that fails its self-check must not write a result.

## Illustrative example

The following illustrative example is schema-valid; its identifiers and
measurements are placeholders, not a benchmark result.

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
    "commit": "0123456789abcdef0123456789abcdef01234567",
    "images": {
      "projectbench-runner": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    },
    "host_cpu": "Apple M2",
    "sut_cpuset": "0-1",
    "sut_memory_mb": 384,
    "generator_cpu_peak_percent": 0,
    "docker": "Docker Engine 27.3.1",
    "os": "Darwin 24.6.0"
  },
  "limitations": []
}
```
