export type ProjectSummary = {
  id: string;
  name: string;
  environment: string;
  latest_run_id: string | null;
};
export type BaselineSummary = {
  project_id: string;
  environment_id: string;
  scenario_id: string;
  run_id: string;
  selected_at: string;
  selected_by: string;
};
export type Environment = {
  id: string;
  project_id: string;
  name: string;
  base_url: string;
  allowed_origins: string[];
  enabled: boolean;
};
export type Scenario = {
  id: string;
  project_id: string;
  name: string;
  version: string;
  script_path: string;
  config_snapshot: { routes?: string[]; [key: string]: unknown };
  config_sha256: string;
  secret_refs: string[];
};
export type PreflightResponse = {
  approved: boolean;
  target_origin: string;
  workload: {
    vus: number;
    duration_seconds: number;
    think_time_seconds: number;
  };
  scenario_hash: string;
  warnings: string[];
};
export type TimeBucket = {
  start_at: string;
  request_count: number;
  failed_count: number;
  duration_count: number;
  duration_sum_ms: number;
  duration_max_ms: number | null;
};
export type CompareResponse = {
  baseline_run_id: string;
  candidate_run_id: string;
  compatible: boolean;
  incompatibility_reasons: string[];
  metrics: {
    name: string;
    baseline: number | null;
    candidate: number | null;
    absolute: number | null;
    relative_percent: number | null;
  }[];
  limitations: string[];
};

export type RunSummary = {
  id: string;
  project_id: string;
  environment_id: string;
  scenario_id: string;
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
  scenario: {
    name: string;
    version: string;
    script_path: string;
    routes: string[];
  };
  workload: {
    vus: number;
    duration_seconds: number;
    think_time_seconds: number;
  };
  synthetic_traffic: boolean;
  metrics: {
    request_count: number;
    throughput_per_second: number;
    error_rate: number;
    p50_ms: number | null;
    p90_ms: number | null;
    p95_ms: number | null;
    p99_ms: number | null;
  };
  thresholds: {
    name: string;
    passed: boolean;
    observed: number | null;
    limit: number | null;
  }[];
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
  scenario: {
    name: string;
    version: string;
    script_path: string;
    routes: string[];
  };
  workload: {
    vus: number;
    duration_seconds: number;
    think_time_seconds: number;
  };
  confirmed: boolean;
};

export type CreatedRun = {
  id: string;
  status: string;
  scenario_hash: string;
  synthetic_traffic: boolean;
};

const apiOrigin = import.meta.env.VITE_API_ORIGIN ?? "";
let operatorToken: string | null = null;

export function setOperatorToken(token: string | null) {
  operatorToken = token?.trim() || null;
}

async function get<T>(path: string): Promise<T> {
  const response = operatorToken
    ? await fetch(`${apiOrigin}${path}`, {
        headers: { Authorization: `Bearer ${operatorToken}` },
      })
    : await fetch(`${apiOrigin}${path}`);
  if (!response.ok) throw new Error(`Metrics API returned ${response.status}`);
  return response.json() as Promise<T>;
}

export const api = {
  projects: () => get<ProjectSummary[]>("/api/v1/projects"),
  createProject: (body: { name: string; description: string }) =>
    post<{ id: string }>("/api/v1/projects", body),
  environments: (projectId: string) =>
    get<Environment[]>(`/api/v1/projects/${projectId}/environments`),
  createEnvironment: (
    projectId: string,
    body: { name: string; base_url: string; allowed_origins: string[] },
  ) => post<Environment>(`/api/v1/projects/${projectId}/environments`, body),
  scenarios: (projectId: string) =>
    get<Scenario[]>(`/api/v1/projects/${projectId}/scenarios`),
  createScenario: (
    projectId: string,
    body: {
      name: string;
      version: string;
      script_path: string;
      config_snapshot: Record<string, unknown>;
      secret_refs: string[];
    },
  ) => post<Scenario>(`/api/v1/projects/${projectId}/scenarios`, body),
  baseline: (projectId: string) =>
    get<BaselineSummary>(`/api/v1/projects/${projectId}/baselines`),
  runs: () => get<RunSummary[]>("/api/v1/runs"),
  run: (id: string) => get<RunDetail>(`/api/v1/runs/${id}`),
  timeSeries: (id: string) =>
    get<TimeBucket[]>(`/api/v1/runs/${id}/timeseries`),
  compare: (baselineId: string, candidateId: string) =>
    get<CompareResponse>(`/api/v1/runs/${baselineId}/compare/${candidateId}`),
  preflight: (request: {
    target_origin: string;
    scenario: RunReport["scenario"];
    workload: RunReport["workload"];
  }) => post<PreflightResponse>("/api/v1/runs/preflight", request),
  artifactUrl: (id: string, kind: string) =>
    `${apiOrigin}/api/v1/runs/${id}/artifacts/${encodeURIComponent(kind)}`,
  createRun: async (request: CreateRunRequest) => {
    const response = await fetch(`${apiOrigin}/api/v1/runs`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(operatorToken ? { Authorization: `Bearer ${operatorToken}` } : {}),
      },
      body: JSON.stringify(request),
    });
    if (!response.ok)
      throw new Error(`Metrics API returned ${response.status}`);
    return response.json() as Promise<CreatedRun>;
  },
  cancelRun: async (id: string) => {
    const response = await fetch(`${apiOrigin}/api/v1/runs/${id}/cancel`, {
      method: "POST",
      headers: operatorToken
        ? { Authorization: `Bearer ${operatorToken}` }
        : undefined,
    });
    if (!response.ok)
      throw new Error(`Metrics API returned ${response.status}`);
  },
  executeRun: async (id: string) => {
    const response = await fetch(`${apiOrigin}/api/v1/runs/${id}/execute`, {
      method: "POST",
      headers: operatorToken
        ? { Authorization: `Bearer ${operatorToken}` }
        : undefined,
    });
    if (!response.ok)
      throw new Error(`Metrics API returned ${response.status}`);
  },
  selectBaseline: async (
    projectId: string,
    request: { environment_id: string; scenario_id: string; run_id: string },
  ) =>
    post<BaselineSummary>(`/api/v1/projects/${projectId}/baselines`, request),
};

async function post<T>(path: string, body: unknown): Promise<T> {
  const response = await fetch(`${apiOrigin}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(operatorToken ? { Authorization: `Bearer ${operatorToken}` } : {}),
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) throw new Error(`Metrics API returned ${response.status}`);
  return response.json() as Promise<T>;
}
