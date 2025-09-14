-- Add base_url column to llm_providers table
-- This migration adds the missing base_url column that the model expects

ALTER TABLE llm_providers ADD COLUMN IF NOT EXISTS base_url TEXT;

-- Add comment for documentation
COMMENT ON COLUMN llm_providers.base_url IS 'Base URL for local LLMs or custom endpoints';
