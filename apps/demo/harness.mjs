import { randomUUID } from "node:crypto";
import { cpuUsage, platform } from "node:process";
import { performance } from "node:perf_hooks";
import { collectHealthObservations } from "./observe.mjs";

const apiBase = process.env.API_BASE_URL;
const sampleUrl = process.env.SAMPLE_URL;
const token = process.env.METRICS_OPERATOR_TOKEN;
const imageDigest = process.env.DEMO_IMAGE_DIGEST;
const claimId = "projectbench-demo-health";
const project = "projectbench";
const workload = "local_health";
const iterations = 5;
const sleep = (duration) =>
  new Promise((resolve) => setTimeout(resolve, duration));

for (const [name, value] of Object.entries({
  apiBase,
  sampleUrl,
  token,
  imageDigest,
})) {
  if (!value) throw new Error(`${name} must be set by the demo stack`);
}

async function waitForApi() {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    try {
      const response = await fetch(`${apiBase}/ready`, {
        signal: AbortSignal.timeout(1000),
      });
      if (response.ok) return;
    } catch {
      // The API may still be applying migrations.
    }
    await sleep(1000);
  }
  throw new Error("ProjectBench API did not become ready within 30 seconds");
}

await waitForApi();
const selfCheck = await fetch(sampleUrl, {
  signal: AbortSignal.timeout(3000),
});
const selfCheckBody = await selfCheck.json();
if (selfCheck.status !== 200 || selfCheckBody.status !== "ok") {
  throw new Error("Demo harness self-check failed; no results were ingested");
}

const resultSetId = randomUUID();
const { results: observations, generatorCpuPercent } = await collectHealthObservations({
  iterations,
  request: () => fetch(sampleUrl, { signal: AbortSignal.timeout(3000) }),
  sleep,
  now: () => performance.now(),
  cpuUsage,
  timestamp: () => new Date().toISOString(),
});
process.stdout.write(`Generator CPU over demo window: ${generatorCpuPercent.toFixed(2)}%\n`);
const results = [];

for (const observation of observations) {
  const result = {
    schema_version: "1",
    claim_id: claimId,
    project,
    variant: "observed",
    workload,
    repetition: observation.repetition,
    started_at: observation.startedAt,
    duration_seconds: observation.durationSeconds,
    metrics: [
      {
        name: "response_success",
        unit: "boolean (1=yes, 0=no)",
        value: observation.succeeded ? 1 : 0,
      },
      {
        name: "response_status_code",
        unit: "HTTP status code",
        value: observation.status,
      },
    ],
    environment: {
      commit: process.env.DEMO_COMMIT || "local-demo",
      images: { "projectbench-demo": imageDigest },
      host_cpu: process.env.DEMO_HOST_CPU || platform,
      sut_cpuset: "0",
      sut_memory_mb: 256,
      generator_cpu_peak_percent: generatorCpuPercent,
      docker: `Docker Engine ${process.env.DEMO_DOCKER_VERSION || "version unavailable"}`,
      os: process.env.DEMO_OS || platform,
    },
    limitations: [
      "Bundled local behavior demo; this result does not measure production behavior or performance.",
      "Generator CPU is sampled across the complete five-request demo window, not per request.",
    ],
  };
  results.push(result);
  process.stdout.write(
    `Observed ${observation.repetition}/${iterations}: HTTP ${observation.status} in ${observation.durationSeconds * 1000} ms\n`,
  );
}

for (const [index, result] of results.entries()) {
  const stored = await fetch(`${apiBase}/api/v1/results`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
      "x-projectbench-result-set": resultSetId,
    },
    body: JSON.stringify(result),
  });
  if (!stored.ok)
    throw new Error(
      `Result ${index + 1} was rejected: HTTP ${stored.status} ${await stored.text()}`,
    );
}

const verdictResponse = await fetch(
  `${apiBase}/api/v1/claims/${claimId}/verdict`,
  {
    headers: { authorization: `Bearer ${token}` },
  },
);
if (!verdictResponse.ok)
  throw new Error(`Verdict request failed: HTTP ${verdictResponse.status}`);
const history = await verdictResponse.json();
const verdict = history.verdict;
process.stdout.write(`\nVERDICT: ${verdict.outcome}\n`);
process.stdout.write(`Next step: ${verdict.next_step}\n`);
if (verdict.workloads?.[0]?.candidate) {
  process.stdout.write(
    `Observed median metric value: ${verdict.workloads[0].candidate.median} boolean (1=yes, 0=no)\n`,
  );
}
process.stdout.write(`Result set: ${history.result_set_id}\n`);
if (verdict.outcome !== "verified") process.exitCode = 1;
