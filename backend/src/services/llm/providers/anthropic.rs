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

/// Anthropic Claude provider implementation
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
    model: String,
    http_client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.anthropic.com/v1".to_string(),
            model,
            http_client: Client::new(),
        }
    }
    
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
    
    /// Convert unified request to Anthropic format
    fn to_anthropic_request(&self, request: &UnifiedLlmRequest) -> Result<Value, AppError> {
        let mut messages = Vec::new();
        
        // Anthropic uses a different message format
        messages.push(json!({
            "role": "user",
            "content": request.prompt
        }));
        
        let mut body = json!({
            "model": self.model,
            "max_tokens": request.max_tokens.unwrap_or(1000),
            "messages": messages
        });
        
        // Add system prompt if provided
        if let Some(system_prompt) = &request.system_prompt {
            body["system"] = json!(system_prompt);
        }
        
        // Add optional parameters
        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }
        
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }
        
        if let Some(top_k) = request.top_k {
            body["top_k"] = json!(top_k);
        }
        
        if let Some(stop) = &request.stop {
            body["stop_sequences"] = json!(stop);
        }
        
        if let Some(stream) = request.stream {
            body["stream"] = json!(stream);
        }
        
        // Handle thinking mode
        if request.enable_thinking.unwrap_or(false) {
            // Add thinking instruction to system prompt
            let thinking_instruction = "\n\nBefore providing your final answer, please think through this step by step in a <thinking> section. Show your reasoning process.";
            if let Some(existing_system) = body["system"].as_str() {
                body["system"] = json!(format!("{}{}", existing_system, thinking_instruction));
            } else {
                body["system"] = json!(thinking_instruction);
            }
        }
        
        // Add extra parameters
        if let Some(extra_params) = &request.extra_params {
            for (key, value) in extra_params {
                body[key] = value.clone();
            }
        }
        
        Ok(body)
    }
    
    /// Parse Anthropic response to unified format
    fn parse_response(&self, response_data: Value, latency_ms: u64) -> Result<UnifiedLlmResponse, AppError> {
        let content = response_data["content"][0]["text"]
            .as_str()
            .ok_or_else(|| AppError::ExternalServiceError("Invalid Anthropic response format".to_string()))?
            .to_string();
        
        // Extract thinking if present
        let (main_content, thinking) = self.extract_thinking(&content);
        
        let usage = if let Some(usage_obj) = response_data["usage"].as_object() {
            TokenUsage {
                prompt_tokens: usage_obj.get("input_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
                completion_tokens: usage_obj.get("output_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
                total_tokens: None, // Anthropic doesn't provide total directly
            }
        } else {
            TokenUsage {
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
            }
        };
        
        // Calculate total tokens if possible
        let total_tokens = if let (Some(input), Some(output)) = (usage.prompt_tokens, usage.completion_tokens) {
            Some(input + output)
        } else {
            None
        };
        
        let usage = TokenUsage {
            total_tokens,
            ..usage
        };
        
        let stop_reason = response_data["stop_reason"]
            .as_str()
            .map(|s| s.to_string());
        
        let metadata = ResponseMetadata {
            latency_ms: Some(latency_ms),
            finish_reason: stop_reason,
            estimated_cost: self.estimate_cost_from_usage(&usage),
            safety_flags: None,
            extra: None,
        };
        
        Ok(UnifiedLlmResponse {
            content: main_content,
            thinking,
            model: self.model.clone(),
            provider: "Anthropic".to_string(),
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
                let thinking_content = content[thinking_start + 10..thinking_end].trim().to_string();
                let main_content = format!(
                    "{}{}",
                    &content[..thinking_start],
                    &content[thinking_end + 11..]
                ).trim().to_string();
                
                return (main_content, Some(thinking_content));
            }
        }
        
        (content.to_string(), None)
    }
    
    fn estimate_cost_from_usage(&self, usage: &TokenUsage) -> Option<f64> {
        // Anthropic pricing (as of 2024, subject to change)
        let (input_cost_per_1k, output_cost_per_1k) = match self.model.as_str() {
            "claude-3-opus-20240229" => (0.015, 0.075),
            "claude-3-sonnet-20240229" => (0.003, 0.015),
            "claude-3-haiku-20240307" => (0.00025, 0.00125),
            "claude-2.1" => (0.008, 0.024),
            "claude-2.0" => (0.008, 0.024),
            "claude-instant-1.2" => (0.0008, 0.0024),
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
impl LlmProvider for AnthropicProvider {
    fn provider_name(&self) -> &str {
        "Anthropic"
    }
    
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_streaming: true,
            supports_thinking: true, // Through XML tags
            supports_system_prompt: true,
            supports_function_calling: false, // Claude doesn't have native function calling
            supports_vision: self.model.starts_with("claude-3"),
            max_context_length: match self.model.as_str() {
                s if s.starts_with("claude-3") => Some(200000),
                "claude-2.1" => Some(200000),
                "claude-2.0" => Some(100000),
                "claude-instant-1.2" => Some(100000),
                _ => Some(100000),
            },
            max_output_length: Some(4096),
        }
    }
    
    async fn available_models(&self) -> Result<Vec<String>, AppError> {
        // Anthropic doesn't have a public models endpoint, return known models
        Ok(vec![
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
            "claude-2.1".to_string(),
            "claude-2.0".to_string(),
            "claude-instant-1.2".to_string(),
        ])
    }
    
    async fn generate(&self, request: UnifiedLlmRequest) -> Result<UnifiedLlmResponse, AppError> {
        self.validate_request(&request)?;
        
        let start_time = Instant::now();
        let anthropic_request = self.to_anthropic_request(&request)?;
        
        let response = self.http_client
            .post(&format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Anthropic API error: {}", e)))?;
        
        let latency_ms = start_time.elapsed().as_millis() as u64;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("Anthropic API error: {}", error_text)));
        }
        
        let response_data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse Anthropic response: {}", e)))?;
        
        self.parse_response(response_data, latency_ms)
    }
    
    async fn generate_stream(&self, request: UnifiedLlmRequest) -> Result<tokio::sync::mpsc::Receiver<Result<String, AppError>>, AppError> {
        self.validate_request(&request)?;
        
        let mut stream_request = request.clone();
        stream_request.stream = Some(true);
        
        let anthropic_request = self.to_anthropic_request(&stream_request)?;
        
        let response = self.http_client
            .post(&format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Anthropic API error: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("Anthropic API error: {}", error_text)));
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
                                
                                if let Ok(json_data) = serde_json::from_str::<Value>(data) {
                                    if json_data["type"] == "content_block_delta" {
                                        if let Some(text) = json_data["delta"]["text"].as_str() {
                                            if tx.send(Ok(text.to_string())).await.is_err() {
                                                return;
                                            }
                                        }
                                    } else if json_data["type"] == "message_stop" {
                                        return;
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
