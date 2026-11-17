CREATE TABLE claims (
    id TEXT PRIMARY KEY,
    project TEXT NOT NULL,
    definition JSONB NOT NULL,
    loaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE bench_results (
    id UUID PRIMARY KEY,
    result_set_id UUID NOT NULL,
    claim_id TEXT NOT NULL REFERENCES claims(id),
    project TEXT NOT NULL,
    variant TEXT NOT NULL,
    workload TEXT NOT NULL,
    repetition INTEGER NOT NULL CHECK (repetition > 0),
    started_at TIMESTAMPTZ NOT NULL,
    document JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (result_set_id, claim_id, variant, workload, repetition)
);

CREATE INDEX bench_results_claim_set_idx
    ON bench_results (claim_id, result_set_id, started_at DESC);
