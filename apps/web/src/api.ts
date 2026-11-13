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
  createRun: async (request: CreateRunRequest) => {
    const response = await fetch(`${apiOrigin}/api/v1/runs`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(request) });
    if (!response.ok) throw new Error(`Metrics API returned ${response.status}`);
    return response.json() as Promise<CreatedRun>;
  },
};

