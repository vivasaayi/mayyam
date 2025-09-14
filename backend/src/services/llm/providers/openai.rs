use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Instant;
use std::collections::HashMap;

use crate::services::llm::interface::{
    LlmProvider, UnifiedLlmRequest, UnifiedLlmResponse, 
    ProviderCapabilities, TokenUsage, ResponseMetadata
};
use crate::errors::AppError;

/// OpenAI provider implementation
#[derive(Debug, Clone)]
pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
    model: String,
    http_client: Client,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            model,
            http_client: Client::new(),
        }
    }
    
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
    
    /// Convert unified request to OpenAI format
    fn to_openai_request(&self, request: &UnifiedLlmRequest) -> Result<Value, AppError> {
        let mut messages = Vec::new();
        
        // Add system prompt if provided
        if let Some(system_prompt) = &request.system_prompt {
            messages.push(json!({
                "role": "system",
                "content": system_prompt
            }));
        }
        
        // Add user prompt
        messages.push(json!({
            "role": "user",
            "content": request.prompt
        }));
        
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
        
        // Handle thinking mode for o1 models
        if request.enable_thinking.unwrap_or(false) && self.model.starts_with("o1") {
            // For o1 models, thinking is built-in
            // No special parameter needed
        }
        
        // Add extra parameters
        if let Some(extra_params) = &request.extra_params {
            for (key, value) in extra_params {
                body[key] = value.clone();
            }
        }
        
        Ok(body)
    }
    
    /// Parse OpenAI response to unified format
    fn parse_response(&self, response_data: Value, latency_ms: u64) -> Result<UnifiedLlmResponse, AppError> {
        let content = response_data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AppError::ExternalServiceError("Invalid OpenAI response format".to_string()))?
            .to_string();
        
        // Extract thinking for o1 models
        let thinking = if self.model.starts_with("o1") {
            // o1 models may include reasoning in a separate field
            response_data["choices"][0]["message"]["reasoning"]
                .as_str()
                .map(|s| s.to_string())
        } else {
            None
        };
        
        let usage = if let Some(usage_obj) = response_data["usage"].as_object() {
            TokenUsage {
                prompt_tokens: usage_obj.get("prompt_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
                completion_tokens: usage_obj.get("completion_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
                total_tokens: usage_obj.get("total_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
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
            content,
            thinking,
            model: self.model.clone(),
            provider: "OpenAI".to_string(),
            usage,
            timestamp: chrono::Utc::now(),
            metadata,
            raw_response: Some(response_data),
        })
    }
    
    fn estimate_cost_from_usage(&self, usage: &TokenUsage) -> Option<f64> {
        // OpenAI pricing (as of 2024, subject to change)
        let (input_cost_per_1k, output_cost_per_1k) = match self.model.as_str() {
            "gpt-4" => (0.03, 0.06),
            "gpt-4-32k" => (0.06, 0.12),
            "gpt-4-turbo-preview" => (0.01, 0.03),
            "gpt-3.5-turbo" => (0.0015, 0.002),
            "gpt-3.5-turbo-16k" => (0.003, 0.004),
            _ => return None,
        };
        
        if let (Some(prompt_tokens), Some(completion_tokens)) = (usage.prompt_tokens, usage.completion_tokens) {
            let input_cost = (prompt_tokens as f64 / 1000.0) * input_cost_per_1k;
            let output_cost = (completion_tokens as f64 / 1000.0) * output_cost_per_1k;
            Some(input_cost + output_cost)
        } else {
            None
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn provider_name(&self) -> &str {
        "OpenAI"
    }
    
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_streaming: true,
            supports_thinking: self.model.starts_with("o1"),
            supports_system_prompt: true,
            supports_function_calling: true,
            supports_vision: self.model.contains("vision") || self.model == "gpt-4",
            max_context_length: match self.model.as_str() {
                "gpt-4-32k" => Some(32768),
                "gpt-3.5-turbo-16k" => Some(16384),
                "gpt-4-turbo-preview" => Some(128000),
                _ => Some(8192),
            },
            max_output_length: Some(4096),
        }
    }
    
    async fn available_models(&self) -> Result<Vec<String>, AppError> {
        let response = self.http_client
            .get(&format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("OpenAI API error: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(AppError::ExternalServiceError("Failed to fetch OpenAI models".to_string()));
        }
        
        let data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse models response: {}", e)))?;
        
        let models = data["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|model| model["id"].as_str())
            .filter(|id| id.starts_with("gpt-"))
            .map(|id| id.to_string())
            .collect();
        
        Ok(models)
    }
    
    async fn generate(&self, request: UnifiedLlmRequest) -> Result<UnifiedLlmResponse, AppError> {
        self.validate_request(&request)?;
        
        let start_time = Instant::now();
        let openai_request = self.to_openai_request(&request)?;
        
        let response = self.http_client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("OpenAI API error: {}", e)))?;
        
        let latency_ms = start_time.elapsed().as_millis() as u64;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("OpenAI API error: {}", error_text)));
        }
        
        let response_data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse OpenAI response: {}", e)))?;
        
        self.parse_response(response_data, latency_ms)
    }
    
    async fn generate_stream(&self, request: UnifiedLlmRequest) -> Result<tokio::sync::mpsc::Receiver<Result<String, AppError>>, AppError> {
        self.validate_request(&request)?;
        
        let mut stream_request = request.clone();
        stream_request.stream = Some(true);
        
        let openai_request = self.to_openai_request(&stream_request)?;
        
        let response = self.http_client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("OpenAI API error: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("OpenAI API error: {}", error_text)));
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
                                    if let Some(content) = json_data["choices"][0]["delta"]["content"].as_str() {
                                        if tx.send(Ok(content.to_string())).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(AppError::ExternalServiceError(format!("Stream error: {}", e)))).await;
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
