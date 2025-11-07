use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

use crate::errors::AppError;
use crate::services::llm::interface::{
    LlmProvider, ProviderCapabilities, ResponseMetadata, TokenUsage, UnifiedLlmRequest,
    UnifiedLlmResponse,
};

/// Provider implementation for locally hosted ChatGPT-compatible models such as Ollama.
#[derive(Debug, Clone)]
pub struct LocalChatGptProvider {
    base_url: String,
    model: String,
    http_client: Client,
}

impl LocalChatGptProvider {
    /// Creates a new local provider targeting the given base URL and model name.
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .pool_max_idle_per_host(16)
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            http_client: client,
        }
    }

    /// Updates the default model used by the provider.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    fn chat_endpoint(&self) -> String {
        format!("{}/api/chat", self.base_url)
    }

    fn tags_endpoint(&self) -> String {
        format!("{}/api/tags", self.base_url)
    }

    fn build_payload(&self, request: &UnifiedLlmRequest) -> Value {
        let mut messages = Vec::new();

        if let Some(system_prompt) = &request.system_prompt {
            messages.push(json!({ "role": "system", "content": system_prompt }));
        }

        messages.push(json!({ "role": "user", "content": request.prompt }));

        let mut payload = json!({
            "model": request
                .extra_params
                .as_ref()
                .and_then(|params| params.get("model"))
                .and_then(|value| value.as_str())
                .unwrap_or(&self.model),
            "messages": messages,
            "stream": request.stream.unwrap_or(false),
        });

        let mut options = serde_json::Map::new();
        if let Some(temp) = request.temperature {
            options.insert("temperature".to_string(), json!(temp));
        }
        if let Some(max_tokens) = request.max_tokens {
            options.insert("num_predict".to_string(), json!(max_tokens));
        }
        if let Some(top_p) = request.top_p {
            options.insert("top_p".to_string(), json!(top_p));
        }
        if let Some(top_k) = request.top_k {
            options.insert("top_k".to_string(), json!(top_k));
        }
        if !options.is_empty() {
            payload["options"] = Value::Object(options);
        }

        if let Some(extra) = &request.extra_params {
            for (key, value) in extra {
                if key == "model" {
                    continue;
                }
                payload[key] = value.clone();
            }
        }

        if let Some(stop) = &request.stop {
            payload["stop"] = json!(stop);
        }

        payload
    }

    fn parse_response(
        &self,
        response_data: Value,
        latency_ms: u64,
    ) -> Result<UnifiedLlmResponse, AppError> {
        let message = response_data
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                AppError::ExternalServiceError(
                    "Invalid response from local LLM provider".to_string(),
                )
            })?;

        let model = response_data
            .get("model")
            .and_then(|value| value.as_str())
            .unwrap_or(&self.model)
            .to_string();

        let metadata = ResponseMetadata {
            latency_ms: Some(latency_ms),
            finish_reason: response_data
                .get("done_reason")
                .and_then(|value| value.as_str())
                .map(|s| s.to_string()),
            estimated_cost: None,
            safety_flags: None,
            extra: response_data
                .get("metrics")
                .and_then(|value| value.as_object())
                .map(|map| map.clone().into_iter().collect::<HashMap<String, Value>>()),
        };

        Ok(UnifiedLlmResponse {
            content: message.to_string(),
            thinking: None,
            model,
            provider: "LocalChatGPT".to_string(),
            usage: TokenUsage {
                prompt_tokens: response_data
                    .get("prompt_eval_count")
                    .and_then(|value| value.as_u64())
                    .map(|value| value as u32),
                completion_tokens: response_data
                    .get("eval_count")
                    .and_then(|value| value.as_u64())
                    .map(|value| value as u32),
                total_tokens: response_data
                    .get("total_tokens")
                    .and_then(|value| value.as_u64())
                    .map(|value| value as u32)
                    .or_else(|| {
                        let prompt = response_data
                            .get("prompt_eval_count")
                            .and_then(|value| value.as_u64())
                            .map(|value| value as u32)
                            .unwrap_or(0);
                        let completion = response_data
                            .get("eval_count")
                            .and_then(|value| value.as_u64())
                            .map(|value| value as u32)
                            .unwrap_or(0);
                        Some(prompt + completion)
                    }),
            },
            timestamp: chrono::Utc::now(),
            metadata,
            raw_response: Some(response_data),
        })
    }
}

#[async_trait]
impl LlmProvider for LocalChatGptProvider {
    fn provider_name(&self) -> &str {
        "LocalChatGPT"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_streaming: true,
            supports_thinking: false,
            supports_system_prompt: true,
            supports_function_calling: false,
            supports_vision: false,
            max_context_length: None,
            max_output_length: None,
        }
    }

    async fn available_models(&self) -> Result<Vec<String>, AppError> {
        let response = self
            .http_client
            .get(self.tags_endpoint())
            .send()
            .await
            .map_err(|error| {
                AppError::ExternalServiceError(format!("Local LLM API error: {}", error))
            })?;

        if !response.status().is_success() {
            return Err(AppError::ExternalServiceError(format!(
                "Failed to fetch local models: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|error| {
            AppError::ExternalServiceError(format!(
                "Failed to parse local models response: {}",
                error
            ))
        })?;

        let models = data
            .get("models")
            .and_then(|value| value.as_array())
            .map(|models| {
                models
                    .iter()
                    .filter_map(|model| model.get("name").and_then(|value| value.as_str()))
                    .map(|name| name.to_string())
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        Ok(models)
    }

    async fn generate(&self, request: UnifiedLlmRequest) -> Result<UnifiedLlmResponse, AppError> {
        self.validate_request(&request)?;

        let mut payload = self.build_payload(&request);
        payload["stream"] = Value::Bool(false);

        let start_time = Instant::now();
        let response = self
            .http_client
            .post(self.chat_endpoint())
            .json(&payload)
            .send()
            .await
            .map_err(|error| {
                AppError::ExternalServiceError(format!("Local LLM API error: {}", error))
            })?;

        let latency_ms = start_time.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown provider error".to_string());
            return Err(AppError::ExternalServiceError(error_text));
        }

        let response_data: Value = response.json().await.map_err(|error| {
            AppError::ExternalServiceError(format!(
                "Failed to parse local provider response: {}",
                error
            ))
        })?;

        self.parse_response(response_data, latency_ms)
    }

    async fn generate_stream(
        &self,
        request: UnifiedLlmRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AppError>>, AppError> {
        self.validate_request(&request)?;

        let mut payload = self.build_payload(&request);
        payload["stream"] = Value::Bool(true);

        let response = self
            .http_client
            .post(self.chat_endpoint())
            .json(&payload)
            .send()
            .await
            .map_err(|error| {
                AppError::ExternalServiceError(format!("Local LLM API error: {}", error))
            })?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown provider error".to_string());
            return Err(AppError::ExternalServiceError(error_text));
        }

        let (tx, rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();

                            if line.is_empty() {
                                continue;
                            }

                            match serde_json::from_str::<Value>(&line) {
                                Ok(json_line) => {
                                    if json_line
                                        .get("done")
                                        .and_then(|value| value.as_bool())
                                        .unwrap_or(false)
                                    {
                                        return;
                                    }

                                    if let Some(content) = json_line
                                        .get("message")
                                        .and_then(|value| value.get("content"))
                                        .and_then(|value| value.as_str())
                                    {
                                        if tx.send(Ok(content.to_string())).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                                Err(error) => {
                                    let _ = tx
                                        .send(Err(AppError::ExternalServiceError(format!(
                                            "Failed to parse streaming chunk: {}",
                                            error
                                        ))))
                                        .await;
                                    return;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let _ = tx
                            .send(Err(AppError::ExternalServiceError(format!(
                                "Local provider streaming error: {}",
                                error
                            ))))
                            .await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn estimate_cost(&self, _request: &UnifiedLlmRequest) -> Result<Option<f64>, AppError> {
        Ok(None)
    }
}
