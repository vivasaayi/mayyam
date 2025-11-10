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
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use std::time::Instant;
// use std::collections::HashMap; // not used currently

use crate::errors::AppError;
use crate::services::llm::interface::{
    LlmProvider, ProviderCapabilities, ResponseMetadata, TokenUsage, UnifiedLlmRequest,
    UnifiedLlmResponse,
};

/// DeepSeek provider implementation
/// DeepSeek uses OpenAI-compatible API format
#[derive(Debug, Clone)]
pub struct DeepSeekProvider {
    api_key: String,
    base_url: String,
    model: String,
    http_client: Client,
}

impl DeepSeekProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.deepseek.com/v1".to_string(),
            model,
            http_client: Client::builder()
                .timeout(Duration::from_secs(60))
                .pool_max_idle_per_host(16)
                .tcp_nodelay(true)
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Convert unified request to DeepSeek format (OpenAI-compatible)
    fn to_deepseek_request(&self, request: &UnifiedLlmRequest) -> Result<Value, AppError> {
        let mut messages = Vec::new();

        // Add system prompt if provided
        if let Some(system_prompt) = &request.system_prompt {
            messages.push(json!({
                "role": "system",
                "content": system_prompt
            }));
        }

        // Add thinking prompt if enabled
        if request.enable_thinking.unwrap_or(false) {
            let thinking_prompt = format!(
                "Please think through this step by step. Show your reasoning in a <thinking> section before providing your final answer.\n\n{}",
                request.prompt
            );
            messages.push(json!({
                "role": "user",
                "content": thinking_prompt
            }));
        } else {
            messages.push(json!({
                "role": "user",
                "content": request.prompt
            }));
        }

        let mut body = json!({
            "model": self.model,
            "messages": messages
        });

        // Add optional parameters
        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }

        if let Some(frequency_penalty) = request.frequency_penalty {
            body["frequency_penalty"] = json!(frequency_penalty);
        }

        if let Some(presence_penalty) = request.presence_penalty {
            body["presence_penalty"] = json!(presence_penalty);
        }

        if let Some(stop) = &request.stop {
            body["stop"] = json!(stop);
        }

        if let Some(stream) = request.stream {
            body["stream"] = json!(stream);
        }

        // Add extra parameters
        if let Some(extra_params) = &request.extra_params {
            for (key, value) in extra_params {
                body[key] = value.clone();
            }
        }

        Ok(body)
    }

    /// Parse DeepSeek response to unified format
    fn parse_response(
        &self,
        response_data: Value,
        latency_ms: u64,
    ) -> Result<UnifiedLlmResponse, AppError> {
        let content = response_data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                AppError::ExternalServiceError("Invalid DeepSeek response format".to_string())
            })?
            .to_string();

        // Extract thinking if present
        let (main_content, thinking) = self.extract_thinking(&content);

        let usage = if let Some(usage_obj) = response_data["usage"].as_object() {
            TokenUsage {
                prompt_tokens: usage_obj
                    .get("prompt_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                completion_tokens: usage_obj
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                total_tokens: usage_obj
                    .get("total_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
            }
        } else {
            TokenUsage {
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
            }
        };

        let finish_reason = response_data["choices"][0]["finish_reason"]
            .as_str()
            .map(|s| s.to_string());

        let metadata = ResponseMetadata {
            latency_ms: Some(latency_ms),
            finish_reason,
            estimated_cost: self.estimate_cost_from_usage(&usage),
            safety_flags: None,
            extra: None,
        };

        Ok(UnifiedLlmResponse {
            content: main_content,
            thinking,
            model: self.model.clone(),
            provider: "DeepSeek".to_string(),
            usage,
            timestamp: chrono::Utc::now(),
            metadata,
            raw_response: Some(response_data),
        })
    }

    /// Extract thinking content from response
    fn extract_thinking(&self, content: &str) -> (String, Option<String>) {
        if let Some(thinking_start) = content.find("<thinking>") {
            if let Some(thinking_end) = content.find("</thinking>") {
                let thinking_content = content[thinking_start + 10..thinking_end]
                    .trim()
                    .to_string();
                let main_content = format!(
                    "{}{}",
                    &content[..thinking_start],
                    &content[thinking_end + 11..]
                )
                .trim()
                .to_string();

                return (main_content, Some(thinking_content));
            }
        }

        (content.to_string(), None)
    }

    fn estimate_cost_from_usage(&self, usage: &TokenUsage) -> Option<f64> {
        // DeepSeek pricing (as of 2024, very competitive)
        let (input_cost_per_1k, output_cost_per_1k) = match self.model.as_str() {
            "deepseek-chat" => (0.0014, 0.0028),
            "deepseek-coder" => (0.0014, 0.0028),
            _ => (0.0014, 0.0028), // Default pricing
        };

        if let (Some(prompt_tokens), Some(completion_tokens)) =
            (usage.prompt_tokens, usage.completion_tokens)
        {
            let input_cost = (prompt_tokens as f64 / 1000.0) * input_cost_per_1k;
            let output_cost = (completion_tokens as f64 / 1000.0) * output_cost_per_1k;
            Some(input_cost + output_cost)
        } else {
            None
        }
    }
}

#[async_trait]
impl LlmProvider for DeepSeekProvider {
    fn provider_name(&self) -> &str {
        "DeepSeek"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_streaming: true,
            supports_thinking: true, // Through XML tags
            supports_system_prompt: true,
            supports_function_calling: false, // DeepSeek doesn't support function calling
            supports_vision: false,           // DeepSeek doesn't support vision
            max_context_length: Some(32768),  // DeepSeek models support long context
            max_output_length: Some(4096),
        }
    }

    async fn available_models(&self) -> Result<Vec<String>, AppError> {
        let response = self
            .http_client
            .get(&format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("DeepSeek API error: {}", e)))?;

        if !response.status().is_success() {
            // Fallback to known models if API doesn't support model listing
            return Ok(vec![
                "deepseek-chat".to_string(),
                "deepseek-coder".to_string(),
            ]);
        }

        let data: Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError(format!("Failed to parse models response: {}", e))
        })?;

        let models = data["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|model| model["id"].as_str())
            .map(|id| id.to_string())
            .collect();

        Ok(models)
    }

    async fn generate(&self, request: UnifiedLlmRequest) -> Result<UnifiedLlmResponse, AppError> {
        self.validate_request(&request)?;

        let start_time = Instant::now();
        let deepseek_request = self.to_deepseek_request(&request)?;

        let response = self
            .http_client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&deepseek_request)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("DeepSeek API error: {}", e)))?;

        let latency_ms = start_time.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!(
                "DeepSeek API error: {}",
                error_text
            )));
        }

        let response_data: Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError(format!("Failed to parse DeepSeek response: {}", e))
        })?;

        self.parse_response(response_data, latency_ms)
    }

    async fn generate_stream(
        &self,
        request: UnifiedLlmRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AppError>>, AppError> {
        self.validate_request(&request)?;

        let mut stream_request = request.clone();
        stream_request.stream = Some(true);

        let deepseek_request = self.to_deepseek_request(&stream_request)?;

        let response = self
            .http_client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&deepseek_request)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("DeepSeek API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!(
                "DeepSeek API error: {}",
                error_text
            )));
        }

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to handle streaming response
        tokio::spawn(async move {
            use futures::stream::StreamExt;

            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete lines
                        while let Some(line_end) = buffer.find('\n') {
                            let line = buffer[..line_end].trim().to_string();
                            buffer = buffer[line_end + 1..].to_string();

                            if line.starts_with("data: ") {
                                let data = &line[6..];

                                if data == "[DONE]" {
                                    return;
                                }

                                if let Ok(json_data) = serde_json::from_str::<Value>(data) {
                                    if let Some(content) =
                                        json_data["choices"][0]["delta"]["content"].as_str()
                                    {
                                        if tx.send(Ok(content.to_string())).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(AppError::ExternalServiceError(format!(
                                "Stream error: {}",
                                e
                            ))))
                            .await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn estimate_cost(&self, request: &UnifiedLlmRequest) -> Result<Option<f64>, AppError> {
        // Rough estimation based on prompt length
        let prompt_tokens = request.prompt.len() / 4; // Rough estimation: 4 chars per token
        let max_completion_tokens = request.max_tokens.unwrap_or(1000);

        let usage = TokenUsage {
            prompt_tokens: Some(prompt_tokens as u32),
            completion_tokens: Some(max_completion_tokens),
            total_tokens: Some(prompt_tokens as u32 + max_completion_tokens),
        };

        Ok(self.estimate_cost_from_usage(&usage))
    }
}
