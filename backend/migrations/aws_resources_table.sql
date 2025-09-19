-- Migration: Add aws_resources table
-- Description: Creates the aws_resources table for storing AWS resource information

CREATE TABLE IF NOT EXISTS aws_resources (
    id UUID PRIMARY KEY,
    sync_id UUID NULL,
    account_id VARCHAR(100) NOT NULL,
    profile VARCHAR(100),
    region VARCHAR(50) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    resource_id VARCHAR(255) NOT NULL,
    arn VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    tags JSONB NOT NULL DEFAULT '{}',
    resource_data JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_refreshed TIMESTAMP WITH TIME ZONE NOT NULL,
    CONSTRAINT uniq_sync_arn UNIQUE (sync_id, arn),
    CONSTRAINT uniq_sync_resourcekey UNIQUE (sync_id, account_id, region, resource_type, resource_id)
);

-- Create indexes for better query performance
CREATE INDEX idx_aws_resources_account_id ON aws_resources(account_id);
CREATE INDEX IF NOT EXISTS idx_aws_resources_sync_id ON aws_resources(sync_id);
CREATE INDEX IF NOT EXISTS idx_aws_resources_type_sync ON aws_resources(resource_type, sync_id);
CREATE INDEX idx_aws_resources_resource_type ON aws_resources(resource_type);
CREATE INDEX idx_aws_resources_region ON aws_resources(region);
CREATE INDEX idx_aws_resources_name ON aws_resources(name);
