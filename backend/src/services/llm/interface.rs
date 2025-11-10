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


use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::errors::AppError;

/// Generic LLM request structure that can be shared across all providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedLlmRequest {
    /// The main prompt text
    pub prompt: String,

    /// Optional system prompt for context
    pub system_prompt: Option<String>,

    /// Temperature for response randomness (0.0 - 2.0)
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,

    /// Top-p sampling parameter
    pub top_p: Option<f32>,

    /// Top-k sampling parameter  
    pub top_k: Option<u32>,

    /// Frequency penalty (-2.0 to 2.0)
    pub frequency_penalty: Option<f32>,

    /// Presence penalty (-2.0 to 2.0)
    pub presence_penalty: Option<f32>,

    /// Stop sequences
    pub stop: Option<Vec<String>>,

    /// Enable "thinking" mode for reasoning (if supported)
    pub enable_thinking: Option<bool>,

    /// Streaming response
    pub stream: Option<bool>,

    /// Additional provider-specific parameters
    pub extra_params: Option<HashMap<String, Value>>,

    /// Context variables for template rendering
    pub context: Option<HashMap<String, Value>>,
}

/// Standardized LLM response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedLlmResponse {
    /// The generated content
    pub content: String,

    /// Reasoning/thinking process (if available)
    pub thinking: Option<String>,

    /// Model used for generation
    pub model: String,

    /// Provider name
    pub provider: String,

    /// Tokens used in the request
    pub usage: TokenUsage,

    /// Response timestamp
    pub timestamp: DateTime<Utc>,

    /// Response metadata
    pub metadata: ResponseMetadata,

    /// Raw response from provider (for debugging)
    pub raw_response: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    /// Response latency in milliseconds
    pub latency_ms: Option<u64>,

    /// Finish reason (completed, length, stop, etc.)
    pub finish_reason: Option<String>,

    /// Cost estimation (if available)
    pub estimated_cost: Option<f64>,

    /// Safety flags or content filtering results
    pub safety_flags: Option<HashMap<String, Value>>,

    /// Additional provider-specific metadata
    pub extra: Option<HashMap<String, Value>>,
}

/// Provider capability flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_streaming: bool,
    pub supports_thinking: bool,
    pub supports_system_prompt: bool,
    pub supports_function_calling: bool,
    pub supports_vision: bool,
    pub max_context_length: Option<u32>,
    pub max_output_length: Option<u32>,
}

/// Unified LLM Provider trait
#[async_trait]
pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    /// Provider identifier
    fn provider_name(&self) -> &str;

    /// Get provider capabilities
    fn capabilities(&self) -> ProviderCapabilities;

    /// Available models for this provider
    async fn available_models(&self) -> Result<Vec<String>, AppError>;

    /// Generate response using the unified interface
    async fn generate(&self, request: UnifiedLlmRequest) -> Result<UnifiedLlmResponse, AppError>;

    /// Stream response (if supported)
    async fn generate_stream(
        &self,
        request: UnifiedLlmRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AppError>>, AppError> {
        // Default implementation for providers that don't support streaming
        Err(AppError::BadRequest(
            "Streaming not supported by this provider".to_string(),
        ))
    }

    /// Validate request parameters for this provider
    fn validate_request(&self, request: &UnifiedLlmRequest) -> Result<(), AppError> {
        // Default validation
        if request.prompt.is_empty() {
            return Err(AppError::BadRequest("Prompt cannot be empty".to_string()));
        }

        if let Some(temp) = request.temperature {
            if temp < 0.0 || temp > 2.0 {
                return Err(AppError::BadRequest(
                    "Temperature must be between 0.0 and 2.0".to_string(),
                ));
            }
        }

        if let Some(max_tokens) = request.max_tokens {
            if max_tokens == 0 {
                return Err(AppError::BadRequest(
                    "Max tokens must be greater than 0".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Estimate cost for request (if available)
    async fn estimate_cost(&self, request: &UnifiedLlmRequest) -> Result<Option<f64>, AppError> {
        // Default implementation returns None
        Ok(None)
    }
}

/// Builder pattern for creating LLM requests
#[derive(Debug, Default)]
pub struct LlmRequestBuilder {
    request: UnifiedLlmRequest,
}

impl LlmRequestBuilder {
    pub fn new() -> Self {
        Self {
            request: UnifiedLlmRequest {
                prompt: String::new(),
                system_prompt: None,
                temperature: None,
                max_tokens: None,
                top_p: None,
                top_k: None,
                frequency_penalty: None,
                presence_penalty: None,
                stop: None,
                enable_thinking: None,
                stream: None,
                extra_params: None,
                context: None,
            },
        }
    }

    pub fn prompt<S: Into<String>>(mut self, prompt: S) -> Self {
        self.request.prompt = prompt.into();
        self
    }

    pub fn system_prompt<S: Into<String>>(mut self, system_prompt: S) -> Self {
        self.request.system_prompt = Some(system_prompt.into());
        self
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.request.temperature = Some(temperature);
        self
    }

    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.request.max_tokens = Some(max_tokens);
        self
    }

    pub fn top_p(mut self, top_p: f32) -> Self {
        self.request.top_p = Some(top_p);
        self
    }

    pub fn top_k(mut self, top_k: u32) -> Self {
        self.request.top_k = Some(top_k);
        self
    }

    pub fn enable_thinking(mut self, enable: bool) -> Self {
        self.request.enable_thinking = Some(enable);
        self
    }

    pub fn stream(mut self, stream: bool) -> Self {
        self.request.stream = Some(stream);
        self
    }

    pub fn stop_sequences(mut self, stop: Vec<String>) -> Self {
        self.request.stop = Some(stop);
        self
    }

    pub fn context_var<K: Into<String>, V: Into<Value>>(mut self, key: K, value: V) -> Self {
        if self.request.context.is_none() {
            self.request.context = Some(HashMap::new());
        }
        self.request
            .context
            .as_mut()
            .unwrap()
            .insert(key.into(), value.into());
        self
    }

    pub fn extra_param<K: Into<String>, V: Into<Value>>(mut self, key: K, value: V) -> Self {
        if self.request.extra_params.is_none() {
            self.request.extra_params = Some(HashMap::new());
        }
        self.request
            .extra_params
            .as_mut()
            .unwrap()
            .insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> UnifiedLlmRequest {
        self.request
    }
}

impl Default for UnifiedLlmRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            system_prompt: None,
            temperature: Some(0.7),
            max_tokens: Some(1000),
            top_p: None,
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            enable_thinking: None,
            stream: Some(false),
            extra_params: None,
            context: None,
        }
    }
}
