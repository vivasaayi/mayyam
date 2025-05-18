-- Create AWS accounts table
CREATE TABLE IF NOT EXISTS aws_accounts (
    id UUID PRIMARY KEY,
    account_id VARCHAR(20) NOT NULL UNIQUE,
    account_name VARCHAR(100) NOT NULL,
    profile VARCHAR(100),
    default_region VARCHAR(20) NOT NULL,
    use_role BOOLEAN NOT NULL DEFAULT FALSE,
    role_arn VARCHAR(255),
    external_id VARCHAR(255),
    access_key_id VARCHAR(255),
    secret_access_key VARCHAR(255),
    last_synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Create index for faster lookups
CREATE INDEX IF NOT EXISTS aws_accounts_account_id_idx ON aws_accounts(account_id);

-- Add comment to table
COMMENT ON TABLE aws_accounts IS 'Stores AWS account credentials and configuration for resource management';
