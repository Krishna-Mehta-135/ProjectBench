ALTER TABLE scenarios
    ADD COLUMN secret_refs JSONB NOT NULL DEFAULT '[]'::jsonb;
