# Write a harness

A harness measures one claim, workload, variant, and repetition at a time. It must exercise the local application, validate its own result, and write schema-v1 JSON only after every self-check passes.

## Before measurement

- Start the app and dependencies from a Compose profile with seeded data and production-like limits.
- Keep the benchmark generator separate from the system under test where possible.
- Record the commit, image IDs/digests, host, Docker version, operating system, CPU allocation, memory limit, and generator CPU peak.
- Use the same setup for baseline and candidate runs. Interleave variants when the comparison is sensitive to order or host load.

## Self-checks

Run a known input before the repetitions. Check that the harness observes the known output, all clients converge when collaboration is involved, and the number of samples is correct. If any check fails, write no result files.

## Result files

Write one strict schema-v1 document per repetition. Include the claim metric and its unit. Use an RFC 3339 timestamp, one-based repetition numbers, real durations and values, and limitations that apply to this run. Never store credentials, prompts, completions, customer data, or private host paths.

See [result-schema.md](result-schema.md) for field definitions and [methodology.md](methodology.md) for comparison rules.
