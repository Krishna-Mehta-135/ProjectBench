# Report format v1

Reports are JSON documents with `schema_version: "1.0"`. Required provenance includes run ID and status, UTC start and finish timestamps, target origin, scenario name/version/script/hash, effective workload, synthetic-traffic indicator, metric definitions and values, threshold results, artifact references with hashes, and limitations.

Reports must not contain credentials, cookies, prompt/completion content, or raw user data. Raw k6 output is retained separately and is not automatically suitable for public sharing.

