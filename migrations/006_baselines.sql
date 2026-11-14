CREATE TABLE baselines (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    environment_id UUID NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    scenario_id UUID NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    run_id UUID NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    selected_at TIMESTAMPTZ NOT NULL,
    selected_by TEXT NOT NULL,
    PRIMARY KEY (project_id, environment_id, scenario_id)
);

CREATE TABLE baseline_history (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL,
    environment_id UUID NOT NULL,
    scenario_id UUID NOT NULL,
    run_id UUID NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    selected_at TIMESTAMPTZ NOT NULL,
    selected_by TEXT NOT NULL
);

CREATE INDEX baseline_history_lookup_idx ON baseline_history (project_id, environment_id, scenario_id, selected_at DESC);
