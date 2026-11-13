ALTER TABLE runs
    ADD COLUMN execution_attempts INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN failure_reason TEXT;

CREATE INDEX runs_running_started_at_idx
    ON runs (started_at)
    WHERE status = 'running';
