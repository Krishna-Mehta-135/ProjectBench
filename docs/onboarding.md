# Project onboarding

Metrics onboards an application through the versioned registry APIs. The core service does not contain product-specific target logic.

## Safe configuration flow

1. Create or update the project metadata with the protected project API.
2. Add an HTTPS environment whose base origin appears in its allowlist.
3. Import or create an immutable scenario version with its relative script path, configuration hash, routes, and nonsecret `secret_refs` names.
4. Keep the environment disabled until the target owner reviews the origin, account, seeded data, workload, hard stop, and rollback criteria.
5. Export the registry definition to verify that it contains no secret values, then review a preflight response before creating a run.

The import endpoint creates a new project identity transactionally. Secret-reference names identify deployment-provided environment variables; the values are never stored in PostgreSQL, exported, logged, or included in reports. Only names declared by the immutable scenario are passed to k6.

## Adding a third project

Use the same flow for any additional application, including Knowdex, with placeholders until the application owner supplies the actual contract:

```json
{
  "schema_version": "1",
  "project": {"name": "third-project", "description": "owner-approved description"},
  "environments": [{
    "name": "staging",
    "base_url": "https://approved-origin.example",
    "allowed_origins": ["https://approved-origin.example"],
    "enabled": false
  }],
  "scenarios": [{
    "name": "read-only-smoke",
    "version": "1",
    "script_path": "k6/scenarios/third-project-read.js",
    "config_snapshot": {"routes": ["GET /owner-approved-route"]},
    "config_sha256": "sha256:computed-from-canonical-config",
    "secret_refs": ["OWNER_APPROVED_SECRET_NAME"]
  }]
}
```

Do not replace placeholders or enable the environment until the exact HTTPS origin, authentication contract, dedicated test account, seeded data, allowed routes, workload, execution window, and stop criteria have been reviewed. CI and automation never contact the placeholder origin.
