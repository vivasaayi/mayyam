-- Create table for multiple models per LLM provider
CREATE TABLE IF NOT EXISTS llm_provider_models (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES llm_providers(id) ON DELETE CASCADE,
    model_name VARCHAR(255) NOT NULL,
    model_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_llm_provider_models_provider ON llm_provider_models(provider_id);
CREATE INDEX IF NOT EXISTS idx_llm_provider_models_enabled ON llm_provider_models(enabled);
