# Concepts

## Claim

A claim is a versioned statement with a project, source, metric, definition, variants or expected behavior, workloads, repetition count, and status. `claims/claims.yaml` is the ledger. A statement without valid local results remains unverified or measuring.

## Result set

A result describes one repetition for one claim, variant, and workload. Schema v1 requires a timestamp, numeric metrics with units, environment details, and limitations. All repetitions from one harness invocation share a result-set ID so the verdict and history stay grouped.

## Verdict

ProjectBench evaluates stored results with fixed rules. Comparative claims use medians and ranges, minimum repetitions, matching environments, and generator CPU limits. Behavior claims compare observed values with the expected minimum. Outcomes include verified, revised, conditional, not supported, and inconclusive. Each verdict includes a next step. No model chooses the outcome or changes a measured value.

## Evidence

Publishing writes a strict allowlist of claim details, workload summaries, every sample, environment, and limitations. Source references, operator data, private IDs, and host paths are excluded. The public site reads those static JSON files; it does not call the private API.
