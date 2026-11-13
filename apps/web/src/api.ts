export type ProjectSummary = {
  id: string;
  name: string;
  environment: string;
  latest_run_id: string | null;
};

export type RunSummary = {
  id: string;
  status: string;
  synthetic_traffic: boolean;
  report_available: boolean;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  failure_reason: string | null;
};

export type RunReport = {
  schema_version: string;
  run_id: string;
  status: string;
  target_origin: string;
  scenario: { name: string; version: string; script_path: string; routes: string[] };
  workload: { vus: number; duration_seconds: number; think_time_seconds: number };
  synthetic_traffic: boolean;
  metrics: { request_count: number; throughput_per_second: number; error_rate: number; p50_ms: number | null; p90_ms: number | null; p95_ms: number | null; p99_ms: number | null };
  thresholds: { name: string; passed: boolean; observed: number | null; limit: number | null }[];
  artifacts: { kind: string; path: string; sha256: string }[];
  limitations: string[];
};

export type RunDetail = {
  id: string;
  status: string;
  target_origin: string;
  workload: RunReport["workload"];
  scenario_snapshot: RunReport["scenario"];
  report: RunReport | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  failure_reason: string | null;
};

export type CreateRunRequest = {
  project_id: string;
  environment_id: string;
  scenario_id: string;
  target_origin: string;
  scenario: { name: string; version: string; script_path: string; routes: string[] };
  workload: { vus: number; duration_seconds: number; think_time_seconds: number };
  confirmed: boolean;
};

export type CreatedRun = { id: string; status: string; scenario_hash: string; synthetic_traffic: boolean };

const apiOrigin = import.meta.env.VITE_API_ORIGIN ?? "";

async function get<T>(path: string): Promise<T> {
  const response = await fetch(`${apiOrigin}${path}`);
  if (!response.ok) throw new Error(`Metrics API returned ${response.status}`);
  return response.json() as Promise<T>;
}

export const api = {
  projects: () => get<ProjectSummary[]>("/api/v1/projects"),
  runs: () => get<RunSummary[]>("/api/v1/runs"),
  run: (id: string) => get<RunDetail>(`/api/v1/runs/${id}`),
  artifactUrl: (id: string, kind: string) => `${apiOrigin}/api/v1/runs/${id}/artifacts/${encodeURIComponent(kind)}`,
  createRun: async (request: CreateRunRequest) => {
    const response = await fetch(`${apiOrigin}/api/v1/runs`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(request) });
    if (!response.ok) throw new Error(`Metrics API returned ${response.status}`);
    return response.json() as Promise<CreatedRun>;
  },
  cancelRun: async (id: string) => {
    const response = await fetch(`${apiOrigin}/api/v1/runs/${id}/cancel`, { method: "POST" });
    if (!response.ok) throw new Error(`Metrics API returned ${response.status}`);
  },
};

