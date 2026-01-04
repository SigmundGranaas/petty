-- Jobs table for async PDF generation
CREATE TABLE IF NOT EXISTS jobs (
    id UUID PRIMARY KEY,
    template VARCHAR(255) NOT NULL,
    data JSONB NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',

    -- Timestamps (use TIMESTAMPTZ for Rust DateTime<Utc> compatibility)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    -- Result
    download_url TEXT,
    file_size BIGINT,
    error_message TEXT,

    -- Metadata
    callback_url TEXT,

    -- Indexes for efficient querying
    CONSTRAINT valid_status CHECK (status IN ('pending', 'processing', 'completed', 'failed'))
);

-- Index for dequeue operations (get oldest pending job)
CREATE INDEX IF NOT EXISTS idx_jobs_pending ON jobs(created_at) WHERE status = 'pending';

-- Index for status lookups
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status, created_at);

-- Index for cleanup queries (find expired results)
CREATE INDEX IF NOT EXISTS idx_jobs_completed_at ON jobs(completed_at) WHERE status = 'completed';

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

DROP TRIGGER IF EXISTS update_jobs_updated_at ON jobs;
CREATE TRIGGER update_jobs_updated_at BEFORE UPDATE ON jobs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
