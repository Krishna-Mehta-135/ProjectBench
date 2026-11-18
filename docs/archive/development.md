# Development

Metrics is developed in small issue branches and reviewed through pull requests.

## Local principles

- Keep credentials in environment variables or a secret store.
- Do not commit production data, cookies, raw user data, or generated run artifacts.
- Do not run load tests against Azure without an explicit target and workload review.
- Keep CI limited to formatting, linting, build, and unit checks; CI must not run a live load test.

The initial implementation will add the Rust workspace, database migrations, k6 profiles, and web client incrementally.
