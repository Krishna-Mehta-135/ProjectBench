# Add a project

1. Add each measurable claim to `claims/claims.yaml`. Give it a stable slug, project slug, source, metric and definition, direction, variants or expected behavior, named workloads, repetition count, status, and a reproduction command.
2. Create a `bench/` directory in the application repository. Use Docker Compose to run a seeded local system with stated CPU and memory limits. Never point a harness at production.
3. Write a harness that exercises the actual local application and emits one schema-v1 JSON result per repetition. Use real observations; do not create fixture measurements or estimate missing fields.
4. Run the harness and check its self-tests: correct known inputs, expected sample count, client convergence where relevant, and no output when a check fails.
5. Ingest the result folder:

   ```sh
   make ingest RESULTS_DIR=/path/to/result-files
   ```

6. Read the claim's latest verdict and history from the operator API. Review the comparison, ranges, resource limits, and limitations.
7. Ask the owner to review the wording and confirm publishing. Publish a selected result set only after that checkpoint.

The supported result format is documented in [result-schema.md](result-schema.md). See [write-a-harness.md](write-a-harness.md) for the harness contract.
