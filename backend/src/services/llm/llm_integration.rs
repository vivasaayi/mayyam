use std::sync::Arc;
use serde_json::{Value, json};
use reqwest::Client;
use std::time::Duration;
use chrono::Utc;

use crate::models::llm_provider::{LlmProviderModel};
use crate::repositories::llm_provider::LlmProviderRepository;
use crate::repositories::prompt_template::PromptTemplateRepository;
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub variables: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub tokens_used: Option<u32>,
    pub model: String,
    pub provider: String,
    pub timestamp: chrono::DateTime<Utc>,
}

#[derive(Debug)]
pub struct LlmIntegrationService {
    llm_provider_repo: Arc<LlmProviderRepository>,
    prompt_template_repo: Arc<PromptTemplateRepository>,
    http_client: Client,
}

impl LlmIntegrationService {
    pub fn new(
        llm_provider_repo: Arc<LlmProviderRepository>,
        prompt_template_repo: Arc<PromptTemplateRepository>,
    ) -> Self {
        Self {
            llm_provider_repo,
            prompt_template_repo,
            http_client: Client::builder()
                .timeout(Duration::from_secs(30))
                .pool_max_idle_per_host(8)
                .tcp_nodelay(true)
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub async fn generate_response(
        &self,
        provider_id: uuid::Uuid,
        request: LlmRequest,
    ) -> Result<LlmResponse, AppError> {
        let provider = self.llm_provider_repo
            .find_by_id(provider_id)
            .await?
            .ok_or_else(|| AppError::NotFound("LLM provider not found".to_string()))?;

        match provider.provider_type.as_str() {
            "openai" => self.call_openai(&provider, request).await,
            "ollama" => self.call_ollama(&provider, request).await,
            "anthropic" => self.call_anthropic(&provider, request).await,
            "local" => self.call_local(&provider, request).await,
            "gemini" => self.call_gemini(&provider, request).await,
            "custom" => self.call_custom(&provider, request).await,
            _ => Err(AppError::BadRequest(format!("Unsupported provider type: {}", provider.provider_type))),
        }
    }

    pub async fn render_prompt_template(
        &self,
        template_id: uuid::Uuid,
        variables: Option<Value>,
    ) -> Result<String, AppError> {
        let template = self.prompt_template_repo
            .find_by_id(&template_id)
            .await?;

        self.render_template(&template.prompt_template, variables)
    }

    pub async fn generate_with_template(
        &self,
        provider_id: uuid::Uuid,
        template_id: uuid::Uuid,
        variables: Option<Value>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<LlmResponse, AppError> {
        let rendered_prompt = self.render_prompt_template(template_id, variables.clone()).await?;
        
        // TODO: Increment usage count
        // self.prompt_template_repo.increment_usage(template_id).await?;

        let request = LlmRequest {
            prompt: rendered_prompt,
            system_prompt: None, // Could be fetched from a system prompt template
            temperature,
            max_tokens,
            variables,
        };

        self.generate_response(provider_id, request).await
    }

    fn render_template(&self, template: &str, variables: Option<Value>) -> Result<String, AppError> {
        let mut rendered = template.to_string();
        
        if let Some(vars) = variables {
            if let Value::Object(map) = vars {
                for (key, value) in map {
                    let placeholder = format!("{{{{{}}}}}", key); // {{variable_name}}
                    let replacement = match value {
                        Value::String(s) => s,
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Array(arr) => serde_json::to_string(&arr)
                            .map_err(|e| AppError::BadRequest(format!("Invalid array variable: {}", e)))?,
                        Value::Object(obj) => serde_json::to_string(&obj)
                            .map_err(|e| AppError::BadRequest(format!("Invalid object variable: {}", e)))?,
                        Value::Null => "null".to_string(),
                    };
                    rendered = rendered.replace(&placeholder, &replacement);
                }
            }
        }
        
        Ok(rendered)
    }

    async fn call_openai(&self, provider: &LlmProviderModel, request: LlmRequest) -> Result<LlmResponse, AppError> {
        let api_key = self.llm_provider_repo.get_decrypted_api_key(provider).await?
            .ok_or_else(|| AppError::BadRequest("OpenAI API key not configured".to_string()))?;

        let default_endpoint = "https://api.openai.com/v1/chat/completions".to_string();
        let endpoint = provider.base_url.as_ref()
            .unwrap_or(&default_endpoint);

        let mut messages = Vec::new();
        
        if let Some(system_prompt) = request.system_prompt {
            messages.push(json!({
                "role": "system",
                "content": system_prompt
            }));
        }
        
        messages.push(json!({
            "role": "user",
            "content": request.prompt
        }));

        let mut body = json!({
            "model": provider.model_name,
            "messages": messages
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        // Merge provider model config
        if let Value::Object(config_obj) = &provider.model_config {
            if let Value::Object(body_obj) = &mut body {
                for (key, value) in config_obj {
                    body_obj.insert(key.clone(), value.clone());
                }
            }
        }

        let response = self.http_client
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("OpenAI API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("OpenAI API error: {}", error_text)));
        }

        let response_data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse OpenAI response: {}", e)))?;

        let content = response_data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AppError::ExternalServiceError("Invalid OpenAI response format".to_string()))?
            .to_string();

        let tokens_used = response_data["usage"]["total_tokens"].as_u64().map(|t| t as u32);

        Ok(LlmResponse {
            content,
            tokens_used,
            model: provider.model_name.clone(),
            provider: "OpenAI".to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn call_ollama(&self, provider: &LlmProviderModel, request: LlmRequest) -> Result<LlmResponse, AppError> {
        let default_endpoint = "http://localhost:11434/api/generate".to_string();
        let endpoint = provider.base_url.as_ref()
            .unwrap_or(&default_endpoint);

        let mut prompt = request.prompt;
        if let Some(system_prompt) = request.system_prompt {
            prompt = format!("System: {}\n\nUser: {}", system_prompt, prompt);
        }

        let mut body = json!({
            "model": provider.model_name,
            "prompt": prompt,
            "stream": false
        });

        if let Some(temp) = request.temperature {
            body["options"] = json!({
                "temperature": temp
            });
        }

        // Merge provider model config
        if let Value::Object(config_obj) = &provider.model_config {
            if let Value::Object(body_obj) = &mut body {
                for (key, value) in config_obj {
                    body_obj.insert(key.clone(), value.clone());
                }
            }
        }

        let response = self.http_client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Ollama API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("Ollama API error: {}", error_text)));
        }

        let response_data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse Ollama response: {}", e)))?;

        let content = response_data["response"]
            .as_str()
            .ok_or_else(|| AppError::ExternalServiceError("Invalid Ollama response format".to_string()))?
            .to_string();

        Ok(LlmResponse {
            content,
            tokens_used: None, // Ollama doesn't typically return token counts
            model: provider.model_name.clone(),
            provider: "Ollama".to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn call_anthropic(&self, provider: &LlmProviderModel, request: LlmRequest) -> Result<LlmResponse, AppError> {
        let api_key = self.llm_provider_repo.get_decrypted_api_key(provider).await?
            .ok_or_else(|| AppError::BadRequest("Anthropic API key not configured".to_string()))?;

        let default_endpoint = "https://api.anthropic.com/v1/messages".to_string();
        let endpoint = provider.base_url.as_ref()
            .unwrap_or(&default_endpoint);

        let mut messages = Vec::new();
        messages.push(json!({
            "role": "user",
            "content": request.prompt
        }));

        let mut body = json!({
            "model": provider.model_name,
            "max_tokens": request.max_tokens.unwrap_or(1000),
            "messages": messages
        });

        if let Some(system_prompt) = request.system_prompt {
            body["system"] = json!(system_prompt);
        }

        // Merge provider model config
        if let Value::Object(config_obj) = &provider.model_config {
            if let Value::Object(body_obj) = &mut body {
                for (key, value) in config_obj {
                    body_obj.insert(key.clone(), value.clone());
                }
            }
        }

        let response = self.http_client
            .post(endpoint)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Anthropic API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("Anthropic API error: {}", error_text)));
        }

        let response_data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse Anthropic response: {}", e)))?;

        let content = response_data["content"][0]["text"]
            .as_str()
            .ok_or_else(|| AppError::ExternalServiceError("Invalid Anthropic response format".to_string()))?
            .to_string();

        let tokens_used = response_data["usage"]["output_tokens"].as_u64().map(|t| t as u32);

        Ok(LlmResponse {
            content,
            tokens_used,
            model: provider.model_name.clone(),
            provider: "Anthropic".to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn call_local(&self, provider: &LlmProviderModel, request: LlmRequest) -> Result<LlmResponse, AppError> {
        // For local models, we'll assume they use an OpenAI-compatible API
        let base_url = provider.base_url.as_ref()
            .ok_or_else(|| AppError::BadRequest("Local model endpoint not configured".to_string()))?;
        let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        let mut messages = Vec::new();
        
        if let Some(system_prompt) = request.system_prompt {
            messages.push(json!({
                "role": "system",
                "content": system_prompt
            }));
        }
        
        messages.push(json!({
            "role": "user",
            "content": request.prompt
        }));

        let mut body = json!({
            "model": provider.model_name,
            "messages": messages
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        let response = self.http_client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Local model API error calling {}: {}", endpoint, e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("Local model API error: {}", error_text)));
        }

        let response_data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse local model response: {}", e)))?;

        let content = response_data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AppError::ExternalServiceError("Invalid local model response format".to_string()))?
            .to_string();

        let tokens_used = response_data["usage"]["total_tokens"].as_u64().map(|t| t as u32);

        Ok(LlmResponse {
            content,
            tokens_used,
            model: provider.model_name.clone(),
            provider: "Local".to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn call_gemini(&self, provider: &LlmProviderModel, request: LlmRequest) -> Result<LlmResponse, AppError> {
        let api_key = self.llm_provider_repo.get_decrypted_api_key(provider).await?
            .ok_or_else(|| AppError::BadRequest("Gemini API key not configured".to_string()))?;

        let default_endpoint = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent", provider.model_name);
        let endpoint = provider.base_url.as_ref()
            .unwrap_or(&default_endpoint);

        let mut body = json!({
            "contents": [{
                "parts": [{
                    "text": request.prompt
                }]
            }]
        });

        body["generationConfig"] = provider.model_config.clone();

        let response = self.http_client
            .post(&format!("{}?key={}", endpoint, api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Gemini API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("Gemini API error: {}", error_text)));
        }

        let response_data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse Gemini response: {}", e)))?;

        let content = response_data["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| AppError::ExternalServiceError("Invalid Gemini response format".to_string()))?
            .to_string();

        Ok(LlmResponse {
            content,
            tokens_used: None,
            model: provider.model_name.clone(),
            provider: "Gemini".to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn call_custom(&self, provider: &LlmProviderModel, request: LlmRequest) -> Result<LlmResponse, AppError> {
        let endpoint = provider.base_url.as_ref()
            .ok_or_else(|| AppError::BadRequest("Custom provider endpoint not configured".to_string()))?;

        // For custom providers, we'll use the provider's format specification
        let body = match provider.prompt_format.as_str() {
            "openai" => {
                json!({
                    "model": provider.model_name,
                    "messages": [{"role": "user", "content": request.prompt}],
                    "temperature": request.temperature.unwrap_or(0.7),
                    "max_tokens": request.max_tokens.unwrap_or(1000)
                })
            },
            "anthropic" => {
                json!({
                    "model": provider.model_name,
                    "messages": [{"role": "user", "content": request.prompt}],
                    "max_tokens": request.max_tokens.unwrap_or(1000)
                })
            },
            "custom" => {
                // Use the model_config as the template for custom format
                if provider.model_config.is_null() {
                    json!({ "prompt": request.prompt })
                } else {
                    provider.model_config.clone()
                }
            },
            _ => {
                json!({
                    "prompt": request.prompt
                })
            }
        };

        let mut req_builder = self.http_client
            .post(endpoint)
            .header("Content-Type", "application/json");

        // Add API key if available
        if let Some(api_key) = self.llm_provider_repo.get_decrypted_api_key(provider).await? {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req_builder
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Custom provider API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!("Custom provider API error: {}", error_text)));
        }

        let response_data: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse custom provider response: {}", e)))?;

        // For custom providers, assume the response is in the 'content' field
        // This can be made configurable based on provider settings
        let content = response_data["content"]
            .as_str()
            .or_else(|| response_data["response"].as_str())
            .or_else(|| response_data["text"].as_str())
            .ok_or_else(|| AppError::ExternalServiceError("Invalid custom provider response format".to_string()))?
            .to_string();

        Ok(LlmResponse {
            content,
            tokens_used: None,
            model: provider.model_name.clone(),
            provider: "Custom".to_string(),
            timestamp: Utc::now(),
        })
    }
}
