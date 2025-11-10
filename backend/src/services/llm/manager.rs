// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::llm_provider::LlmProviderModel;
use crate::repositories::llm_provider::LlmProviderRepository;
use crate::services::llm::formatting::{FormattedResponse, ResponseFormatter};
use crate::services::llm::interface::{
    LlmProvider, LlmRequestBuilder, UnifiedLlmRequest, UnifiedLlmResponse,
};
use crate::services::llm::providers::{
    AnthropicProvider, DeepSeekProvider, LocalChatGptProvider, OpenAIProvider,
};

/// Unified LLM Manager - The main interface for all LLM operations
#[derive(Debug)]
pub struct UnifiedLlmManager {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    provider_repo: Arc<LlmProviderRepository>,
    default_formatter: ResponseFormatter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGenerationRequest {
    /// Provider identifier (openai, anthropic, deepseek, etc.)
    pub provider: String,

    /// Model name (gpt-4, claude-3-opus, etc.)
    pub model: Option<String>,

    /// The request parameters
    pub request: UnifiedLlmRequest,

    /// Whether to format the response
    pub format_response: Option<bool>,

    /// Custom formatting options
    pub formatting_options: Option<FormattingOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattingOptions {
    pub strip_markdown: Option<bool>,
    pub extract_code_blocks: Option<bool>,
    pub normalize_whitespace: Option<bool>,
    pub extract_structured_data: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGenerationResponse {
    pub response: UnifiedLlmResponse,
    pub formatted: Option<FormattedResponse>,
    pub provider_info: ProviderInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub model: String,
    pub capabilities: crate::services::llm::interface::ProviderCapabilities,
}

impl UnifiedLlmManager {
    pub fn new(provider_repo: Arc<LlmProviderRepository>) -> Self {
        Self {
            providers: HashMap::new(),
            provider_repo,
            default_formatter: ResponseFormatter::default(),
        }
    }

    /// Register a provider with the manager
    pub fn register_provider(&mut self, name: String, provider: Arc<dyn LlmProvider>) {
        self.providers.insert(name, provider);
    }

    /// Initialize with common providers
    pub async fn initialize_common_providers(&mut self) -> Result<(), AppError> {
        // Get all configured providers from database
        let db_providers = self.provider_repo.find_all().await?;

        for db_provider in db_providers {
            if !db_provider.enabled {
                continue;
            }

            match db_provider.provider_type.as_str() {
                "openai" => {
                    if let Some(api_key) = self
                        .provider_repo
                        .get_decrypted_api_key(&db_provider)
                        .await?
                    {
                        let mut provider =
                            OpenAIProvider::new(api_key, db_provider.model_name.clone());
                        if let Some(base_url) = &db_provider.base_url {
                            provider = provider.with_base_url(base_url.clone());
                        }
                        self.register_provider(db_provider.id.to_string(), Arc::new(provider));
                    }
                }
                "anthropic" => {
                    if let Some(api_key) = self
                        .provider_repo
                        .get_decrypted_api_key(&db_provider)
                        .await?
                    {
                        let mut provider =
                            AnthropicProvider::new(api_key, db_provider.model_name.clone());
                        if let Some(base_url) = &db_provider.base_url {
                            provider = provider.with_base_url(base_url.clone());
                        }
                        self.register_provider(db_provider.id.to_string(), Arc::new(provider));
                    }
                }
                "deepseek" => {
                    if let Some(api_key) = self
                        .provider_repo
                        .get_decrypted_api_key(&db_provider)
                        .await?
                    {
                        let mut provider =
                            DeepSeekProvider::new(api_key, db_provider.model_name.clone());
                        if let Some(base_url) = &db_provider.base_url {
                            provider = provider.with_base_url(base_url.clone());
                        }
                        self.register_provider(db_provider.id.to_string(), Arc::new(provider));
                    }
                }
                "local" | "ollama" => {
                    let base_url = db_provider
                        .base_url
                        .clone()
                        .unwrap_or_else(|| {
                            tracing::warn!(
                                "Local/Ollama provider '{}' missing base URL; defaulting to http://localhost:11434",
                                db_provider.name
                            );
                            "http://localhost:11434".to_string()
                        });

                    let provider =
                        LocalChatGptProvider::new(base_url, db_provider.model_name.clone());
                    self.register_provider(db_provider.id.to_string(), Arc::new(provider));
                }
                _ => {
                    // Skip unsupported provider types
                    tracing::warn!("Unsupported provider type: {}", db_provider.provider_type);
                }
            }
        }

        Ok(())
    }

    /// Generate response using specified provider
    pub async fn generate(
        &self,
        request: LlmGenerationRequest,
    ) -> Result<LlmGenerationResponse, AppError> {
        let provider = self.get_provider(&request.provider)?;

        // Generate response
        let response = provider.generate(request.request.clone()).await?;

        // Format response if requested
        let formatted = if request.format_response.unwrap_or(false) {
            let formatter = self.create_formatter(&request.formatting_options);
            Some(formatter.format(response.clone())?)
        } else {
            None
        };

        Ok(LlmGenerationResponse {
            response,
            formatted,
            provider_info: ProviderInfo {
                name: provider.provider_name().to_string(),
                model: request.model.unwrap_or_default(),
                capabilities: provider.capabilities(),
            },
        })
    }

    /// Generate streaming response
    pub async fn generate_stream(
        &self,
        request: LlmGenerationRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AppError>>, AppError> {
        let provider = self.get_provider(&request.provider)?;
        provider.generate_stream(request.request).await
    }

    /// Smart provider selection based on request characteristics
    pub async fn generate_smart(
        &self,
        request: UnifiedLlmRequest,
    ) -> Result<LlmGenerationResponse, AppError> {
        let provider_name = self.select_best_provider(&request)?;

        let generation_request = LlmGenerationRequest {
            provider: provider_name,
            model: None, // Use provider's default model
            request,
            format_response: Some(true),
            formatting_options: None,
        };

        self.generate(generation_request).await
    }

    /// Get available providers
    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Get provider capabilities
    pub fn get_provider_capabilities(
        &self,
        provider_name: &str,
    ) -> Result<crate::services::llm::interface::ProviderCapabilities, AppError> {
        let provider = self.get_provider(provider_name)?;
        Ok(provider.capabilities())
    }

    /// Estimate cost for request across all providers
    pub async fn estimate_costs(
        &self,
        request: &UnifiedLlmRequest,
    ) -> Result<HashMap<String, Option<f64>>, AppError> {
        let mut costs = HashMap::new();

        for (name, provider) in &self.providers {
            let cost = provider.estimate_cost(request).await?;
            costs.insert(name.clone(), cost);
        }

        Ok(costs)
    }

    /// Create a request builder
    pub fn request_builder() -> LlmRequestBuilder {
        LlmRequestBuilder::new()
    }

    /// Get provider by name
    fn get_provider(&self, name: &str) -> Result<&Arc<dyn LlmProvider>, AppError> {
        self.providers
            .get(name)
            .ok_or_else(|| AppError::NotFound(format!("Provider '{}' not found", name)))
    }

    /// Create formatter with options
    fn create_formatter(&self, options: &Option<FormattingOptions>) -> ResponseFormatter {
        let mut formatter = self.default_formatter.clone();

        if let Some(opts) = options {
            if let Some(strip_markdown) = opts.strip_markdown {
                formatter = formatter.strip_markdown(strip_markdown);
            }
            if let Some(extract_code) = opts.extract_code_blocks {
                formatter = formatter.extract_code_blocks(extract_code);
            }
            if let Some(normalize_whitespace) = opts.normalize_whitespace {
                formatter = formatter.normalize_whitespace(normalize_whitespace);
            }
            if let Some(extract_structured) = opts.extract_structured_data {
                formatter = formatter.extract_structured_data(extract_structured);
            }
        }

        formatter
    }

    /// Smart provider selection logic
    fn select_best_provider(&self, request: &UnifiedLlmRequest) -> Result<String, AppError> {
        if self.providers.is_empty() {
            return Err(AppError::BadRequest("No providers available".to_string()));
        }

        // Simple selection logic - can be made more sophisticated
        let prompt_length = request.prompt.len();
        let needs_thinking = request.enable_thinking.unwrap_or(false);
        let needs_streaming = request.stream.unwrap_or(false);

        // Prefer providers based on request characteristics
        for (name, provider) in &self.providers {
            let capabilities = provider.capabilities();

            // If thinking is needed, prefer providers that support it
            if needs_thinking && !capabilities.supports_thinking {
                continue;
            }

            // If streaming is needed, prefer providers that support it
            if needs_streaming && !capabilities.supports_streaming {
                continue;
            }

            // Check context length limits
            if let Some(max_context) = capabilities.max_context_length {
                if prompt_length > (max_context as usize * 4) {
                    // Rough token estimation
                    continue;
                }
            }

            return Ok(name.clone());
        }

        // Fallback to first available provider
        Ok(self.providers.keys().next().unwrap().clone())
    }
}

/// Convenience functions for quick access
impl UnifiedLlmManager {
    /// Quick text generation with default settings
    pub async fn quick_generate(&self, provider: &str, prompt: &str) -> Result<String, AppError> {
        let request = UnifiedLlmRequest {
            prompt: prompt.to_string(),
            ..Default::default()
        };

        let generation_request = LlmGenerationRequest {
            provider: provider.to_string(),
            model: None,
            request,
            format_response: Some(false),
            formatting_options: None,
        };

        let response = self.generate(generation_request).await?;
        Ok(response.response.content)
    }

    /// Generate with thinking enabled
    pub async fn generate_with_thinking(
        &self,
        provider: &str,
        prompt: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let request = UnifiedLlmRequest {
            prompt: prompt.to_string(),
            enable_thinking: Some(true),
            ..Default::default()
        };

        let generation_request = LlmGenerationRequest {
            provider: provider.to_string(),
            model: None,
            request,
            format_response: Some(false),
            formatting_options: None,
        };

        let response = self.generate(generation_request).await?;
        Ok((response.response.content, response.response.thinking))
    }

    /// Generate with custom temperature
    pub async fn generate_with_temperature(
        &self,
        provider: &str,
        prompt: &str,
        temperature: f32,
    ) -> Result<String, AppError> {
        let request = UnifiedLlmRequest {
            prompt: prompt.to_string(),
            temperature: Some(temperature),
            ..Default::default()
        };

        let generation_request = LlmGenerationRequest {
            provider: provider.to_string(),
            model: None,
            request,
            format_response: Some(false),
            formatting_options: None,
        };

        let response = self.generate(generation_request).await?;
        Ok(response.response.content)
    }
}
