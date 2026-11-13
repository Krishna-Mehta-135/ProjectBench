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

const apiOrigin = import.meta.env.VITE_API_ORIGIN ?? "";

async function get<T>(path: string): Promise<T> {
  const response = await fetch(`${apiOrigin}${path}`);
  if (!response.ok) throw new Error(`Metrics API returned ${response.status}`);
  return response.json() as Promise<T>;
}

export const api = {
  projects: () => get<ProjectSummary[]>("/api/v1/projects"),
  runs: () => get<RunSummary[]>("/api/v1/runs"),
};

