-- AWS Cost Analytics Tables

-- Raw cost data from AWS Cost Explorer API
CREATE TABLE IF NOT EXISTS aws_cost_data (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id VARCHAR(20) NOT NULL,
    service_name VARCHAR(255) NOT NULL,
    usage_type VARCHAR(255),
    operation VARCHAR(255),
    region VARCHAR(50),
    usage_start DATE NOT NULL,
    usage_end DATE NOT NULL,
    unblended_cost DECIMAL(15,4) NOT NULL DEFAULT 0.0,
    blended_cost DECIMAL(15,4) NOT NULL DEFAULT 0.0,
    usage_amount DECIMAL(15,6) DEFAULT 0.0,
    usage_unit VARCHAR(50),
    currency VARCHAR(3) DEFAULT 'USD',
    tags JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Monthly aggregated cost data
CREATE TABLE IF NOT EXISTS aws_monthly_cost_aggregates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id VARCHAR(20) NOT NULL,
    service_name VARCHAR(255) NOT NULL,
    month_year DATE NOT NULL, -- First day of the month
    total_cost DECIMAL(15,4) NOT NULL DEFAULT 0.0,
    usage_amount DECIMAL(15,6) DEFAULT 0.0,
    usage_unit VARCHAR(50),
    cost_change_pct DECIMAL(5,2), -- Percentage change from previous month
    cost_change_amount DECIMAL(15,4), -- Absolute change from previous month
    anomaly_score DECIMAL(5,2), -- Statistical anomaly score (z-score)
    is_anomaly BOOLEAN DEFAULT FALSE,
    tags_summary JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Cost anomalies and alerts
CREATE TABLE IF NOT EXISTS aws_cost_anomalies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id VARCHAR(20) NOT NULL,
    service_name VARCHAR(255) NOT NULL,
    anomaly_type VARCHAR(50) NOT NULL, -- 'spike', 'trend', 'new_service'
    severity VARCHAR(20) NOT NULL, -- 'low', 'medium', 'high', 'critical'
    detected_date DATE NOT NULL,
    anomaly_score DECIMAL(5,2) NOT NULL,
    baseline_cost DECIMAL(15,4),
    actual_cost DECIMAL(15,4) NOT NULL,
    cost_difference DECIMAL(15,4),
    percentage_change DECIMAL(5,2),
    description TEXT,
    status VARCHAR(20) DEFAULT 'open', -- 'open', 'investigating', 'resolved', 'false_positive'
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- LLM-generated cost insights and recommendations
CREATE TABLE IF NOT EXISTS aws_cost_insights (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    anomaly_id UUID REFERENCES aws_cost_anomalies(id),
    aggregate_id UUID REFERENCES aws_monthly_cost_aggregates(id),
    account_id VARCHAR(20) NOT NULL,
    insight_type VARCHAR(50) NOT NULL, -- 'anomaly_analysis', 'trend_analysis', 'recommendation'
    prompt_template TEXT NOT NULL,
    llm_provider VARCHAR(100) NOT NULL,
    llm_model VARCHAR(100) NOT NULL,
    llm_response TEXT NOT NULL,
    summary TEXT,
    recommendations JSONB,
    confidence_score DECIMAL(3,2), -- 0.0 to 1.0
    tokens_used INTEGER,
    processing_time_ms INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Cost forecasts
CREATE TABLE IF NOT EXISTS aws_cost_forecasts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id VARCHAR(20) NOT NULL,
    service_name VARCHAR(255),
    forecast_date DATE NOT NULL,
    forecast_period VARCHAR(20) NOT NULL, -- 'daily', 'weekly', 'monthly'
    predicted_cost DECIMAL(15,4) NOT NULL,
    confidence_interval_lower DECIMAL(15,4),
    confidence_interval_upper DECIMAL(15,4),
    model_type VARCHAR(50) NOT NULL, -- 'linear', 'exponential', 'arima', 'prophet'
    accuracy_score DECIMAL(3,2),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Cost budgets and thresholds
CREATE TABLE IF NOT EXISTS aws_cost_budgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id VARCHAR(20) NOT NULL,
    budget_name VARCHAR(255) NOT NULL,
    service_name VARCHAR(255), -- NULL for account-wide budgets
    budget_type VARCHAR(20) NOT NULL, -- 'monthly', 'quarterly', 'yearly'
    budget_amount DECIMAL(15,4) NOT NULL,
    current_spend DECIMAL(15,4) DEFAULT 0.0,
    threshold_warning DECIMAL(5,2) DEFAULT 80.0, -- Percentage
    threshold_critical DECIMAL(5,2) DEFAULT 95.0, -- Percentage
    is_active BOOLEAN DEFAULT TRUE,
    last_checked TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_aws_cost_data_account_service_date ON aws_cost_data(account_id, service_name, usage_start);
CREATE INDEX IF NOT EXISTS idx_aws_cost_data_service_date ON aws_cost_data(service_name, usage_start);
CREATE INDEX IF NOT EXISTS idx_aws_cost_data_date_cost ON aws_cost_data(usage_start, unblended_cost);

CREATE INDEX IF NOT EXISTS idx_aws_monthly_aggregates_account_service_month ON aws_monthly_cost_aggregates(account_id, service_name, month_year);
CREATE INDEX IF NOT EXISTS idx_aws_monthly_aggregates_anomaly ON aws_monthly_cost_aggregates(is_anomaly, month_year);

CREATE INDEX IF NOT EXISTS idx_aws_cost_anomalies_account_date ON aws_cost_anomalies(account_id, detected_date);
CREATE INDEX IF NOT EXISTS idx_aws_cost_anomalies_severity_status ON aws_cost_anomalies(severity, status);

CREATE INDEX IF NOT EXISTS idx_aws_cost_insights_anomaly ON aws_cost_insights(anomaly_id);
CREATE INDEX IF NOT EXISTS idx_aws_cost_insights_aggregate ON aws_cost_insights(aggregate_id);

-- Views for common queries
CREATE OR REPLACE VIEW aws_cost_monthly_summary AS
SELECT 
    account_id,
    month_year,
    SUM(total_cost) as total_monthly_cost,
    COUNT(DISTINCT service_name) as services_count,
    COUNT(*) FILTER (WHERE is_anomaly = TRUE) as anomalies_count,
    AVG(cost_change_pct) as avg_cost_change_pct
FROM aws_monthly_cost_aggregates
GROUP BY account_id, month_year
ORDER BY month_year DESC;

CREATE OR REPLACE VIEW aws_top_services_current_month AS
SELECT 
    account_id,
    service_name,
    total_cost,
    cost_change_pct,
    ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY total_cost DESC) as cost_rank
FROM aws_monthly_cost_aggregates
WHERE month_year = DATE_TRUNC('month', CURRENT_DATE)
ORDER BY account_id, total_cost DESC;
