-- Chaos Engineering Framework Tables
-- Migration: 018_chaos_engineering.sql

-- ============================================================
-- 1. Chaos Experiment Templates (pre-built experiment blueprints)
-- ============================================================
CREATE TABLE IF NOT EXISTS chaos_experiment_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(100) NOT NULL,          -- e.g., 'compute', 'database', 'networking', 'storage', 'serverless'
    resource_type VARCHAR(100) NOT NULL,      -- e.g., 'EC2Instance', 'RdsInstance', 'LambdaFunction'
    experiment_type VARCHAR(100) NOT NULL,    -- e.g., 'instance_stop', 'failover', 'network_latency'
    default_parameters JSONB NOT NULL DEFAULT '{}',
    -- default_parameters example:
    -- {
    --   "duration_seconds": 60,
    --   "intensity": "medium",
    --   "dry_run": true,
    --   "rollback_on_failure": true
    -- }
    prerequisites TEXT[],                     -- list of prerequisites before running
    expected_impact VARCHAR(50) NOT NULL DEFAULT 'medium',  -- 'low', 'medium', 'high', 'critical'
    estimated_duration_seconds INTEGER NOT NULL DEFAULT 60,
    rollback_steps JSONB NOT NULL DEFAULT '[]',
    documentation TEXT,
    is_built_in BOOLEAN NOT NULL DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 2. Chaos Experiments (user-configured experiments)
-- ============================================================
CREATE TABLE IF NOT EXISTS chaos_experiments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    template_id UUID REFERENCES chaos_experiment_templates(id) ON DELETE SET NULL,
    account_id VARCHAR(100) NOT NULL,         -- AWS account ID
    region VARCHAR(50) NOT NULL,
    resource_type VARCHAR(100) NOT NULL,
    target_resource_id VARCHAR(500) NOT NULL,  -- AWS resource ID (e.g., i-xxxx, arn:aws:rds:...)
    target_resource_name VARCHAR(255),
    experiment_type VARCHAR(100) NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}',
    -- parameters example:
    -- {
    --   "duration_seconds": 120,
    --   "force": false,
    --   "dry_run": false,
    --   "rollback_on_failure": true,
    --   "notification_channels": ["email"],
    --   "custom_params": {}
    -- }
    schedule_cron VARCHAR(100),               -- optional cron schedule
    status VARCHAR(50) NOT NULL DEFAULT 'draft',  -- draft, ready, scheduled, running, completed, failed, cancelled
    created_by VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 3. Chaos Experiment Runs (execution history)
-- ============================================================
CREATE TABLE IF NOT EXISTS chaos_experiment_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    experiment_id UUID NOT NULL REFERENCES chaos_experiments(id) ON DELETE CASCADE,
    run_number INTEGER NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',  -- pending, initializing, running, rolling_back, completed, failed, cancelled, timed_out
    started_at TIMESTAMP WITH TIME ZONE,
    ended_at TIMESTAMP WITH TIME ZONE,
    duration_ms BIGINT,
    triggered_by VARCHAR(255),                 -- user or 'scheduler'
    execution_log JSONB NOT NULL DEFAULT '[]',
    -- execution_log example:
    -- [
    --   {"timestamp": "...", "level": "info", "message": "Starting experiment..."},
    --   {"timestamp": "...", "level": "info", "message": "EC2 instance i-xxxx stopped"},
    --   {"timestamp": "...", "level": "warn", "message": "Rollback initiated"}
    -- ]
    error_message TEXT,
    rollback_status VARCHAR(50),               -- null, 'pending', 'in_progress', 'completed', 'failed'
    rollback_started_at TIMESTAMP WITH TIME ZONE,
    rollback_ended_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 4. Chaos Experiment Results (metrics and observations)
-- ============================================================
CREATE TABLE IF NOT EXISTS chaos_experiment_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES chaos_experiment_runs(id) ON DELETE CASCADE,
    experiment_id UUID NOT NULL REFERENCES chaos_experiments(id) ON DELETE CASCADE,
    resource_id VARCHAR(500) NOT NULL,
    resource_type VARCHAR(100) NOT NULL,
    -- Metrics captured at different phases
    baseline_metrics JSONB NOT NULL DEFAULT '{}',
    -- baseline_metrics example:
    -- {
    --   "cpu_utilization": 45.2,
    --   "memory_utilization": 62.1,
    --   "request_count": 1250,
    --   "error_rate": 0.001,
    --   "latency_p50_ms": 12,
    --   "latency_p99_ms": 55,
    --   "connections_active": 42
    -- }
    during_metrics JSONB NOT NULL DEFAULT '{}',
    recovery_metrics JSONB NOT NULL DEFAULT '{}',
    -- Observations
    impact_summary TEXT,
    impact_severity VARCHAR(20) NOT NULL DEFAULT 'unknown', -- none, low, medium, high, critical, unknown
    recovery_time_ms BIGINT,
    steady_state_hypothesis TEXT,
    hypothesis_met BOOLEAN,
    observations JSONB NOT NULL DEFAULT '[]',
    -- observations example:
    -- [
    --   {"type": "metric_change", "metric": "error_rate", "before": 0.001, "during": 0.15, "after": 0.002},
    --   {"type": "event", "description": "Failover completed in 35 seconds"},
    --   {"type": "alert", "description": "CloudWatch alarm triggered: HighErrorRate"}
    -- ]
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 5. Indexes for efficient querying
-- ============================================================
CREATE INDEX IF NOT EXISTS idx_chaos_templates_resource_type ON chaos_experiment_templates(resource_type);
CREATE INDEX IF NOT EXISTS idx_chaos_templates_category ON chaos_experiment_templates(category);
CREATE INDEX IF NOT EXISTS idx_chaos_templates_experiment_type ON chaos_experiment_templates(experiment_type);

CREATE INDEX IF NOT EXISTS idx_chaos_experiments_account_id ON chaos_experiments(account_id);
CREATE INDEX IF NOT EXISTS idx_chaos_experiments_resource_type ON chaos_experiments(resource_type);
CREATE INDEX IF NOT EXISTS idx_chaos_experiments_target_resource ON chaos_experiments(target_resource_id);
CREATE INDEX IF NOT EXISTS idx_chaos_experiments_status ON chaos_experiments(status);
CREATE INDEX IF NOT EXISTS idx_chaos_experiments_template_id ON chaos_experiments(template_id);

CREATE INDEX IF NOT EXISTS idx_chaos_runs_experiment_id ON chaos_experiment_runs(experiment_id);
CREATE INDEX IF NOT EXISTS idx_chaos_runs_status ON chaos_experiment_runs(status);
CREATE INDEX IF NOT EXISTS idx_chaos_runs_started_at ON chaos_experiment_runs(started_at);

CREATE INDEX IF NOT EXISTS idx_chaos_results_run_id ON chaos_experiment_results(run_id);
CREATE INDEX IF NOT EXISTS idx_chaos_results_experiment_id ON chaos_experiment_results(experiment_id);
CREATE INDEX IF NOT EXISTS idx_chaos_results_resource_id ON chaos_experiment_results(resource_id);

-- ============================================================
-- 6. Seed built-in experiment templates
-- ============================================================

-- EC2 Templates
INSERT INTO chaos_experiment_templates (name, description, category, resource_type, experiment_type, default_parameters, prerequisites, expected_impact, estimated_duration_seconds, rollback_steps, documentation, is_built_in)
VALUES
('EC2 Instance Stop', 'Stop an EC2 instance to test application resilience when compute resources become unavailable', 'compute', 'EC2Instance', 'instance_stop',
 '{"duration_seconds": 300, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['Instance must be in running state', 'Ensure Auto Scaling group is configured for replacement'],
 'high', 300,
 '[{"step": 1, "action": "start_instance", "description": "Restart the stopped EC2 instance"}]',
 'Stops the target EC2 instance using the AWS EC2 StopInstances API. The instance will be started again after the specified duration or on rollback.',
 true),

('EC2 Instance Reboot', 'Reboot an EC2 instance to test recovery from instance restarts', 'compute', 'EC2Instance', 'instance_reboot',
 '{"duration_seconds": 120, "dry_run": false}',
 ARRAY['Instance must be in running state'],
 'medium', 120,
 '[{"step": 1, "action": "wait_for_running", "description": "Wait for instance to return to running state"}]',
 'Reboots the target EC2 instance using the AWS EC2 RebootInstances API.',
 true),

('EC2 Instance Terminate', 'Terminate an EC2 instance to test auto-recovery and auto-scaling behavior', 'compute', 'EC2Instance', 'instance_terminate',
 '{"duration_seconds": 600, "dry_run": false, "rollback_on_failure": false}',
 ARRAY['Instance should be part of an Auto Scaling group', 'Ensure replacement capacity is available'],
 'critical', 600,
 '[]',
 'Terminates the target EC2 instance. This is irreversible - the instance will be permanently destroyed. Only use when Auto Scaling is configured.',
 true),

-- RDS Templates
('RDS Failover', 'Trigger a failover on a Multi-AZ RDS instance to test database resilience', 'database', 'RdsInstance', 'rds_failover',
 '{"duration_seconds": 300, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['RDS instance must be Multi-AZ', 'Instance must be in available state'],
 'high', 300,
 '[{"step": 1, "action": "wait_for_available", "description": "Wait for RDS instance to return to available state"}]',
 'Triggers a failover for a Multi-AZ RDS DB instance using the RebootDBInstance API with ForceFailover. Tests application handling of database endpoint changes.',
 true),

('RDS Instance Reboot', 'Reboot an RDS instance to test application behavior during database restarts', 'database', 'RdsInstance', 'rds_reboot',
 '{"duration_seconds": 300, "force_failover": false, "dry_run": false}',
 ARRAY['Instance must be in available state'],
 'high', 300,
 '[{"step": 1, "action": "wait_for_available", "description": "Wait for RDS instance to return to available state"}]',
 'Reboots the target RDS instance using the RebootDBInstance API.',
 true),

-- DynamoDB Templates
('DynamoDB Table Throttle Simulation', 'Reduce provisioned capacity to simulate throttling', 'database', 'DynamoDbTable', 'dynamodb_throttle',
 '{"duration_seconds": 300, "target_read_capacity": 1, "target_write_capacity": 1, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['Table must use provisioned capacity mode', 'Note original capacity for rollback'],
 'high', 300,
 '[{"step": 1, "action": "restore_capacity", "description": "Restore original provisioned capacity"}]',
 'Reduces DynamoDB table provisioned capacity to minimum values to simulate throttling conditions.',
 true),

-- Lambda Templates
('Lambda Function Concurrency Limit', 'Set reserved concurrency to 0 to effectively disable a Lambda function', 'serverless', 'LambdaFunction', 'lambda_disable',
 '{"duration_seconds": 120, "reserved_concurrency": 0, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['Function must exist and be active'],
 'high', 120,
 '[{"step": 1, "action": "restore_concurrency", "description": "Remove reserved concurrency limit"}]',
 'Sets reserved concurrency to 0, preventing the Lambda function from executing. Tests downstream service resilience.',
 true),

('Lambda Function Timeout Reduction', 'Reduce Lambda timeout to force timeout errors', 'serverless', 'LambdaFunction', 'lambda_timeout',
 '{"duration_seconds": 120, "target_timeout_seconds": 1, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['Function must exist and be active', 'Note original timeout for rollback'],
 'medium', 120,
 '[{"step": 1, "action": "restore_timeout", "description": "Restore original Lambda timeout configuration"}]',
 'Reduces the Lambda function timeout to force timeout errors for invocations that take longer than the new limit.',
 true),

-- ECS Templates
('ECS Service Scale Down', 'Scale down an ECS service to test behavior with reduced capacity', 'compute', 'EcsService', 'ecs_scale_down',
 '{"duration_seconds": 300, "target_count": 0, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['Service must exist and be active', 'Note original desired count for rollback'],
 'high', 300,
 '[{"step": 1, "action": "restore_desired_count", "description": "Restore original ECS service desired count"}]',
 'Scales down the ECS service desired count to test service discovery, load balancing, and upstream handling of reduced capacity.',
 true),

-- ElastiCache Templates
('ElastiCache Failover', 'Trigger a failover on an ElastiCache Redis replication group', 'database', 'ElasticacheCluster', 'elasticache_failover',
 '{"duration_seconds": 300, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['Must be a Redis replication group with Multi-AZ enabled', 'Must have at least one replica'],
 'high', 300,
 '[{"step": 1, "action": "wait_for_available", "description": "Wait for ElastiCache cluster to return to available state"}]',
 'Triggers a failover on an ElastiCache Redis replication group using the TestFailover API.',
 true),

-- S3 Templates
('S3 Bucket Policy Deny', 'Apply a deny-all bucket policy to simulate S3 access failures', 'storage', 'S3Bucket', 's3_deny_access',
 '{"duration_seconds": 120, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['Bucket must exist', 'Save current bucket policy for rollback'],
 'high', 120,
 '[{"step": 1, "action": "restore_bucket_policy", "description": "Restore original S3 bucket policy"}]',
 'Applies a deny-all bucket policy to the target S3 bucket, simulating access failures for all operations.',
 true),

-- ALB Templates
('ALB Target Deregistration', 'Deregister targets from an ALB target group to simulate instance failures', 'networking', 'Alb', 'alb_deregister_targets',
 '{"duration_seconds": 300, "deregister_percentage": 50, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['ALB must have registered targets', 'Note registered targets for rollback'],
 'high', 300,
 '[{"step": 1, "action": "register_targets", "description": "Re-register deregistered targets"}]',
 'Deregisters a percentage of targets from the ALB target group to simulate partial infrastructure failure.',
 true),

-- Security Group Templates
('Security Group Ingress Block', 'Remove all ingress rules from a security group to simulate network isolation', 'networking', 'SecurityGroup', 'sg_block_ingress',
 '{"duration_seconds": 120, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['Security group must exist', 'Save current rules for rollback', 'Ensure you have alternative access (e.g., SSM)'],
 'critical', 120,
 '[{"step": 1, "action": "restore_ingress_rules", "description": "Restore original security group ingress rules"}]',
 'Removes all ingress rules from the target security group, effectively blocking all inbound traffic to associated resources.',
 true),

-- SQS Templates
('SQS Queue Purge', 'Purge all messages from an SQS queue to test message loss handling', 'serverless', 'SqsQueue', 'sqs_purge',
 '{"dry_run": false}',
 ARRAY['Queue must exist', 'Understand that purged messages cannot be recovered'],
 'high', 10,
 '[]',
 'Purges all messages from the target SQS queue using the PurgeQueue API. This is irreversible.',
 true),

-- EKS Templates
('EKS Node Group Scale Down', 'Scale down an EKS managed node group to test pod rescheduling', 'compute', 'EksCluster', 'eks_scale_down',
 '{"duration_seconds": 600, "target_size": 1, "dry_run": false, "rollback_on_failure": true}',
 ARRAY['EKS cluster must have managed node groups', 'Note original node group size for rollback'],
 'high', 600,
 '[{"step": 1, "action": "restore_node_group_size", "description": "Restore original EKS node group desired size"}]',
 'Scales down the EKS managed node group, forcing pod rescheduling and testing cluster resilience.',
 true)

ON CONFLICT DO NOTHING;
