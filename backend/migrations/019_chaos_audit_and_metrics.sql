-- Chaos Engineering Audit Logging & Metrics
-- Migration: 019_chaos_audit_and_metrics.sql

-- ============================================================
-- 1. Chaos Audit Log Table
-- ============================================================
CREATE TABLE IF NOT EXISTS chaos_audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    experiment_id UUID REFERENCES chaos_experiments(id) ON DELETE SET NULL,
    run_id UUID REFERENCES chaos_experiment_runs(id) ON DELETE SET NULL,
    action VARCHAR(50) NOT NULL,  -- created, updated, deleted, run_started, run_completed, run_failed, stopped, rollback_started, rollback_completed
    user_id VARCHAR(255),          -- user who triggered the action
    triggered_by VARCHAR(255),     -- 'ui_user', 'scheduler', 'api', 'cli'
    resource_id VARCHAR(500),      -- AWS resource ID if applicable
    old_values JSONB NOT NULL DEFAULT '{}',
    new_values JSONB NOT NULL DEFAULT '{}',
    status_before VARCHAR(50),
    status_after VARCHAR(50),
    details JSONB NOT NULL DEFAULT '{}',  -- Additional context about the action
    ip_address VARCHAR(45),        -- IPv4 or IPv6
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 2. Chaos Execution Metrics Table
-- ============================================================
CREATE TABLE IF NOT EXISTS chaos_execution_metrics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES chaos_experiment_runs(id) ON DELETE CASCADE,
    experiment_id UUID NOT NULL REFERENCES chaos_experiments(id) ON DELETE CASCADE,
    resource_id VARCHAR(500) NOT NULL,
    resource_type VARCHAR(100) NOT NULL,

    -- Timing metrics
    execution_duration_ms BIGINT,
    rollback_duration_ms BIGINT,
    total_duration_ms BIGINT,

    -- Success metrics
    execution_success BOOLEAN,
    rollback_success BOOLEAN,

    -- Impact metrics
    impact_severity VARCHAR(20),  -- none, low, medium, high, critical
    estimated_affected_resources INTEGER,
    confirmed_affected_resources INTEGER,

    -- Recovery metrics
    time_to_first_error_ms BIGINT,
    time_to_recovery_ms BIGINT,
    recovery_completeness_percent INTEGER,  -- 0-100, how much was recovered

    -- System metrics
    api_calls_made INTEGER,
    api_errors INTEGER,
    retries_performed INTEGER,

    -- Custom metrics (experiment-specific)
    custom_metrics JSONB NOT NULL DEFAULT '{}',
    -- Example: {
    --   "cpu_before": 45.2,
    --   "cpu_during": 2.1,
    --   "cpu_after": 44.8,
    --   "error_rate_before": 0.001,
    --   "error_rate_during": 0.15,
    --   "error_rate_after": 0.002
    -- }

    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 3. Chaos Metrics Aggregates (for dashboards/analytics)
-- ============================================================
CREATE TABLE IF NOT EXISTS chaos_metrics_aggregates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    aggregation_type VARCHAR(50) NOT NULL,  -- daily, weekly, monthly, all_time
    experiment_id UUID REFERENCES chaos_experiments(id) ON DELETE SET NULL,
    resource_type VARCHAR(100),

    -- Aggregated stats
    total_runs BIGINT DEFAULT 0,
    successful_runs BIGINT DEFAULT 0,
    failed_runs BIGINT DEFAULT 0,
    success_rate_percent INTEGER,

    avg_execution_duration_ms BIGINT,
    max_execution_duration_ms BIGINT,
    min_execution_duration_ms BIGINT,

    avg_recovery_time_ms BIGINT,
    max_recovery_time_ms BIGINT,

    avg_impact_severity VARCHAR(20),
    most_common_failure_reason VARCHAR(255),

    rollback_success_rate_percent INTEGER,

    -- Time window
    aggregation_start_at TIMESTAMP WITH TIME ZONE,
    aggregation_end_at TIMESTAMP WITH TIME ZONE,

    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 4. Indexes for efficient querying
-- ============================================================
CREATE INDEX IF NOT EXISTS idx_chaos_audit_logs_experiment_id ON chaos_audit_logs(experiment_id);
CREATE INDEX IF NOT EXISTS idx_chaos_audit_logs_run_id ON chaos_audit_logs(run_id);
CREATE INDEX IF NOT EXISTS idx_chaos_audit_logs_action ON chaos_audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_chaos_audit_logs_user_id ON chaos_audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_chaos_audit_logs_created_at ON chaos_audit_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_chaos_audit_logs_resource_id ON chaos_audit_logs(resource_id);

CREATE INDEX IF NOT EXISTS idx_chaos_execution_metrics_run_id ON chaos_execution_metrics(run_id);
CREATE INDEX IF NOT EXISTS idx_chaos_execution_metrics_experiment_id ON chaos_execution_metrics(experiment_id);
CREATE INDEX IF NOT EXISTS idx_chaos_execution_metrics_resource_id ON chaos_execution_metrics(resource_id);
CREATE INDEX IF NOT EXISTS idx_chaos_execution_metrics_impact_severity ON chaos_execution_metrics(impact_severity);
CREATE INDEX IF NOT EXISTS idx_chaos_execution_metrics_created_at ON chaos_execution_metrics(created_at);

CREATE INDEX IF NOT EXISTS idx_chaos_metrics_aggregates_experiment_id ON chaos_metrics_aggregates(experiment_id);
CREATE INDEX IF NOT EXISTS idx_chaos_metrics_aggregates_aggregation_type ON chaos_metrics_aggregates(aggregation_type);
CREATE INDEX IF NOT EXISTS idx_chaos_metrics_aggregates_resource_type ON chaos_metrics_aggregates(resource_type);
