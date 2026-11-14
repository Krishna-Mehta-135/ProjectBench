CREATE TABLE run_artifacts (
    run_id UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    partial BOOLEAN NOT NULL DEFAULT FALSE,
    retention_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (run_id, kind)
);

CREATE INDEX run_artifacts_retention_idx ON run_artifacts (retention_until);
