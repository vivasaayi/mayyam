-- Create data_sources table
CREATE TABLE data_sources (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    data_source_type VARCHAR(50) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    source_type VARCHAR(50) NOT NULL,
    connection_config JSONB NOT NULL,
    metric_config JSONB,
    thresholds JSONB,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create indexes for data_sources
CREATE INDEX idx_data_sources_resource_type ON data_sources(resource_type);
CREATE INDEX idx_data_sources_source_type ON data_sources(source_type);
CREATE INDEX idx_data_sources_status ON data_sources(status);
CREATE INDEX idx_data_sources_created_at ON data_sources(created_at);

-- Create llm_providers table
CREATE TABLE llm_providers (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    provider_type VARCHAR(50) NOT NULL,
    model_name VARCHAR(255) NOT NULL,
    api_endpoint TEXT,
    encrypted_api_key TEXT,
    model_config JSONB,
    prompt_format VARCHAR(50) NOT NULL DEFAULT 'openai',
    description TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create indexes for llm_providers
CREATE INDEX idx_llm_providers_provider_type ON llm_providers(provider_type);
CREATE INDEX idx_llm_providers_status ON llm_providers(status);
CREATE INDEX idx_llm_providers_created_at ON llm_providers(created_at);

-- Create prompt_templates table
CREATE TABLE prompt_templates (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(50) NOT NULL,
    prompt_type VARCHAR(50) NOT NULL,
    template_content TEXT NOT NULL,
    variables JSONB,
    tags TEXT[],
    version INTEGER NOT NULL DEFAULT 1,
    is_system_prompt BOOLEAN NOT NULL DEFAULT FALSE,
    parent_id UUID REFERENCES prompt_templates(id),
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    usage_count INTEGER NOT NULL DEFAULT 0,
    last_used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create indexes for prompt_templates
CREATE INDEX idx_prompt_templates_name ON prompt_templates(name);
CREATE INDEX idx_prompt_templates_category ON prompt_templates(category);
CREATE INDEX idx_prompt_templates_prompt_type ON prompt_templates(prompt_type);
CREATE INDEX idx_prompt_templates_status ON prompt_templates(status);
CREATE INDEX idx_prompt_templates_version ON prompt_templates(version);
CREATE INDEX idx_prompt_templates_usage_count ON prompt_templates(usage_count);
CREATE INDEX idx_prompt_templates_tags ON prompt_templates USING GIN(tags);
CREATE INDEX idx_prompt_templates_created_at ON prompt_templates(created_at);

-- Create unique constraint for prompt template names and versions
CREATE UNIQUE INDEX idx_prompt_templates_name_version ON prompt_templates(name, version);

-- Create analytics_executions table to track analytics runs
CREATE TABLE analytics_executions (
    id UUID PRIMARY KEY,
    resource_id VARCHAR(255) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    analysis_type VARCHAR(50) NOT NULL,
    data_source_ids UUID[] NOT NULL,
    llm_provider_id UUID REFERENCES llm_providers(id),
    prompt_template_id UUID REFERENCES prompt_templates(id),
    execution_time_ms BIGINT,
    data_points_analyzed INTEGER,
    confidence_score REAL,
    insights_count INTEGER DEFAULT 0,
    recommendations_count INTEGER DEFAULT 0,
    status VARCHAR(50) NOT NULL DEFAULT 'completed',
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create indexes for analytics_executions
CREATE INDEX idx_analytics_executions_resource_id ON analytics_executions(resource_id);
CREATE INDEX idx_analytics_executions_resource_type ON analytics_executions(resource_type);
CREATE INDEX idx_analytics_executions_analysis_type ON analytics_executions(analysis_type);
CREATE INDEX idx_analytics_executions_status ON analytics_executions(status);
CREATE INDEX idx_analytics_executions_created_at ON analytics_executions(created_at);

-- Add some default data sources for common use cases
INSERT INTO data_sources (id, name, description, data_source_type, resource_type, source_type, connection_config) VALUES
(gen_random_uuid(), 'AWS CloudWatch - DynamoDB', 'CloudWatch metrics for DynamoDB tables', 'metrics', 'DynamoDB', 'CloudWatch', '{"namespace": "AWS/DynamoDB", "region": "us-east-1"}'),
(gen_random_uuid(), 'AWS CloudWatch - RDS', 'CloudWatch metrics for RDS instances', 'metrics', 'RDS', 'CloudWatch', '{"namespace": "AWS/RDS", "region": "us-east-1"}'),
(gen_random_uuid(), 'AWS CloudWatch - EKS', 'CloudWatch metrics for EKS clusters', 'metrics', 'Kubernetes', 'CloudWatch', '{"namespace": "AWS/EKS", "region": "us-east-1"}');

-- Add some default prompt templates
INSERT INTO prompt_templates (id, name, description, category, prompt_type, template_content, variables) VALUES
(gen_random_uuid(), 'DynamoDB_performance_analysis', 'Analyze DynamoDB table performance metrics', 'DynamoDB', 'analysis', 
'You are an expert AWS DynamoDB performance analyst. Analyze the following metrics for table {{resource_id}} over the time period {{time_range.start}} to {{time_range.end}}.

Metrics Data:
{{#each metrics}}
- {{name}}: avg={{avg}}, max={{max}}, min={{min}}, count={{count}} {{unit}}
{{/each}}

Please provide:
1. Performance insights and any concerning patterns
2. Specific recommendations for optimization
3. Root cause analysis if issues are detected
4. Confidence score (0-1) for your analysis

Format your response as JSON with the following structure:
{
  "insights": [
    {
      "title": "Insight title",
      "description": "Detailed description",
      "severity": "critical|high|medium|low",
      "category": "performance|capacity|errors",
      "metrics_involved": ["metric1", "metric2"]
    }
  ],
  "recommendations": [
    {
      "title": "Recommendation title", 
      "description": "Detailed description",
      "priority": "immediate|high|medium|low",
      "impact": "Expected impact",
      "action_items": ["action1", "action2"]
    }
  ],
  "summary": "Brief overall summary",
  "confidence_score": 0.9
}',
'{"resource_id": "string", "time_range": {"start": "datetime", "end": "datetime"}, "metrics": "array"}'),

(gen_random_uuid(), 'RDS_performance_analysis', 'Analyze RDS instance performance metrics', 'Database', 'analysis',
'You are an expert RDS database performance analyst. Analyze the following metrics for RDS instance {{resource_id}} over the time period {{time_range.start}} to {{time_range.end}}.

Metrics Data:
{{#each metrics}}
- {{name}}: avg={{avg}}, max={{max}}, min={{min}}, count={{count}} {{unit}}
{{/each}}

Please provide:
1. Database performance insights and any concerning patterns
2. Specific recommendations for optimization
3. Root cause analysis if issues are detected
4. Confidence score (0-1) for your analysis

Format your response as JSON with the same structure as DynamoDB analysis.',
'{"resource_id": "string", "time_range": {"start": "datetime", "end": "datetime"}, "metrics": "array"}'),

(gen_random_uuid(), 'Kubernetes_performance_analysis', 'Analyze Kubernetes cluster performance metrics', 'Kubernetes', 'analysis',
'You are an expert Kubernetes performance analyst. Analyze the following metrics for cluster {{resource_id}} over the time period {{time_range.start}} to {{time_range.end}}.

Metrics Data:
{{#each metrics}}
- {{name}}: avg={{avg}}, max={{max}}, min={{min}}, count={{count}} {{unit}}
{{/each}}

Please provide:
1. Cluster performance insights and any concerning patterns
2. Specific recommendations for optimization
3. Root cause analysis if issues are detected
4. Confidence score (0-1) for your analysis

Format your response as JSON with the same structure as previous analyses.',
'{"resource_id": "string", "time_range": {"start": "datetime", "end": "datetime"}, "metrics": "array"}');

-- Update updated_at timestamp automatically
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_data_sources_updated_at BEFORE UPDATE ON data_sources 
FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_llm_providers_updated_at BEFORE UPDATE ON llm_providers 
FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_prompt_templates_updated_at BEFORE UPDATE ON prompt_templates 
FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
