# Quickstart

ProjectBench's bundled demo needs Docker Engine with Compose v2, Make, and Git. No external app credentials or API keys are required.

From a clean checkout, run:

```sh
make demo
```

The demo builds the Rust API, PostgreSQL, the public web app, and a tiny local health endpoint. A Node harness makes five real HTTP requests to that endpoint, writes schema-v1 results from the observed status and response body, ingests them into PostgreSQL, and prints the fixed verdict and next step. It does not call CanvasSync, Knowdex, Gemini, or a production endpoint.

Open the public site at `http://127.0.0.1:4174`. It reads static evidence JSON only, so it remains in the empty state until an owner reviews and publishes evidence. The local operator API is at `http://127.0.0.1:18080`; its default demo token is `projectbench-demo-token` and is bound to loopback.

To change the token or host ports, copy `.env.example` to `.env`. The demo uses a dedicated Compose project and leaves the database volume in place when stopped:

```sh
make demo-down
```

To reset the demo database as well:

```sh
docker compose -p projectbench-demo --profile demo down -v
```

The demo result is a behavior check for the bundled service. It does not measure production latency or prove application performance.
