-- Add missing columns to llm_providers table to match SeaORM model expectations
ALTER TABLE llm_providers ADD COLUMN IF NOT EXISTS api_key TEXT;
COMMENT ON COLUMN llm_providers.api_key IS 'API key for external LLM providers';

ALTER TABLE llm_providers ADD COLUMN IF NOT EXISTS enabled BOOLEAN NOT NULL DEFAULT TRUE;
COMMENT ON COLUMN llm_providers.enabled IS 'Whether this LLM provider is enabled';

ALTER TABLE llm_providers ADD COLUMN IF NOT EXISTS is_default BOOLEAN NOT NULL DEFAULT FALSE;
COMMENT ON COLUMN llm_providers.is_default IS 'Whether this is the default LLM provider';

-- Rename api_endpoint to base_url if it exists and base_url doesn't
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'llm_providers' AND column_name = 'api_endpoint')
    AND NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'llm_providers' AND column_name = 'base_url') THEN
        ALTER TABLE llm_providers RENAME COLUMN api_endpoint TO base_url;
    END IF;
END $$;

-- Migrate data from encrypted_api_key to api_key if encrypted_api_key exists
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'llm_providers' AND column_name = 'encrypted_api_key')
    AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'llm_providers' AND column_name = 'api_key') THEN
        UPDATE llm_providers SET api_key = encrypted_api_key WHERE api_key IS NULL AND encrypted_api_key IS NOT NULL;
    END IF;
END $$;

-- Set default values for enabled and is_default based on status
UPDATE llm_providers SET enabled = CASE WHEN status = 'active' THEN TRUE ELSE FALSE END WHERE enabled IS NULL;
UPDATE llm_providers SET is_default = FALSE WHERE is_default IS NULL;
