-- Create table to track sync runs/sessions
CREATE TABLE IF NOT EXISTS sync_runs (
    id UUID PRIMARY KEY, -- sync_id
    name TEXT NOT NULL,
    aws_account_id UUID NULL, -- references aws_accounts.id (soft reference; no FK to avoid migration order issues)
    account_id TEXT NULL, -- AWS 12-digit account id (optional redundancy)
    profile TEXT NULL,
    region TEXT NULL,
    status TEXT NOT NULL DEFAULT 'created', -- created|running|completed|failed
    total_resources INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    error_summary TEXT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    started_at TIMESTAMPTZ NULL,
    completed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Helpful indexes
CREATE INDEX IF NOT EXISTS idx_sync_runs_created_at ON sync_runs (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_runs_status ON sync_runs (status);

-- Trigger to auto-update updated_at
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_sync_runs_updated_at ON sync_runs;
CREATE TRIGGER trg_sync_runs_updated_at
BEFORE UPDATE ON sync_runs
FOR EACH ROW
EXECUTE PROCEDURE set_updated_at();
