-- Add missing columns to aws_accounts table
-- This migration ensures the database matches the SeaORM model

ALTER TABLE aws_accounts ADD COLUMN IF NOT EXISTS regions JSONB;
ALTER TABLE aws_accounts ADD COLUMN IF NOT EXISTS auth_type VARCHAR(50) NOT NULL DEFAULT 'auto';
ALTER TABLE aws_accounts ADD COLUMN IF NOT EXISTS source_profile VARCHAR(100);
ALTER TABLE aws_accounts ADD COLUMN IF NOT EXISTS sso_profile VARCHAR(100);
ALTER TABLE aws_accounts ADD COLUMN IF NOT EXISTS web_identity_token_file VARCHAR(255);
ALTER TABLE aws_accounts ADD COLUMN IF NOT EXISTS session_name VARCHAR(100);

-- Update existing records to have 'auto' auth_type if NULL (though DEFAULT handles new ones)
UPDATE aws_accounts SET auth_type = 'auto' WHERE auth_type IS NULL;
