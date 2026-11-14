CREATE TABLE run_time_buckets (
    run_id UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    bucket_start TIMESTAMPTZ NOT NULL,
    request_count BIGINT NOT NULL CHECK (request_count >= 0),
    failed_count BIGINT NOT NULL CHECK (failed_count >= 0),
    duration_count BIGINT NOT NULL CHECK (duration_count >= 0),
    duration_sum_ms DOUBLE PRECISION NOT NULL CHECK (duration_sum_ms >= 0),
    duration_max_ms DOUBLE PRECISION,
    PRIMARY KEY (run_id, bucket_start)
);
