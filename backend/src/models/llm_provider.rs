use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;

/// LLM provider configuration entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "llm_providers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub provider_type: String, // "openai", "ollama", "anthropic", "local", etc.
    pub base_url: Option<String>, // For local LLMs or custom endpoints
    pub api_key: Option<String>, // For external providers
    pub model_name: String,
    pub model_config: Json, // Model-specific configuration
    pub prompt_format: String, // How to format prompts for this LLM
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// Domain model for LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderDomain {
    pub id: Uuid,
    pub name: String,
    pub provider_type: LlmProviderType,
    pub base_url: Option<String>,
    pub api_key: Option<String>, // This should be hidden in responses
    pub model_name: String,
    pub model_config: LlmModelConfig,
    pub prompt_format: LlmPromptFormat,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LlmProviderType {
    OpenAI,
    Ollama,
    Anthropic,
    Local,
    Gemini,
    DeepSeek,
    Custom,
}

impl From<String> for LlmProviderType {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Self::OpenAI,
            "ollama" => Self::Ollama,
            "anthropic" => Self::Anthropic,
            "local" => Self::Local,
            "gemini" => Self::Gemini,
            "deepseek" => Self::DeepSeek,
            _ => Self::Custom,
        }
    }
}

impl From<LlmProviderType> for String {
    fn from(provider_type: LlmProviderType) -> Self {
        match provider_type {
            LlmProviderType::OpenAI => "openai".to_string(),
            LlmProviderType::Ollama => "ollama".to_string(),
            LlmProviderType::Anthropic => "anthropic".to_string(),
            LlmProviderType::Local => "local".to_string(),
            LlmProviderType::Gemini => "gemini".to_string(),
            LlmProviderType::DeepSeek => "deepseek".to_string(),
            LlmProviderType::Custom => "custom".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LlmProviderStatus {
    Active,
    Inactive,
    Error,
    Pending,
}

impl From<String> for LlmProviderStatus {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            "error" => Self::Error,
            "pending" => Self::Pending,
            _ => Self::Active,
        }
    }
}

impl From<LlmProviderStatus> for String {
    fn from(status: LlmProviderStatus) -> Self {
        match status {
            LlmProviderStatus::Active => "active".to_string(),
            LlmProviderStatus::Inactive => "inactive".to_string(),
            LlmProviderStatus::Error => "error".to_string(),
            LlmProviderStatus::Pending => "pending".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModelConfig {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub timeout_seconds: Option<u32>,
    pub custom_parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmPromptFormat {
    OpenAI,          // {"role": "user", "content": "..."}
    Ollama,          // Simple string format
    Anthropic,       // Human/Assistant format
    Custom,          // Add missing Custom variant
    CustomTemplate { template: String }, // Custom template with placeholders
}

/// DTO for creating new LLM provider
#[derive(Debug, Deserialize)]
pub struct LlmProviderCreateDto {
    pub name: String,
    pub provider_type: LlmProviderType,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: String,
    pub model_config: LlmModelConfig,
    pub prompt_format: LlmPromptFormat,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

/// DTO for updating LLM provider
#[derive(Debug, Deserialize)]
pub struct LlmProviderUpdateDto {
    pub name: Option<String>,
    pub provider_type: Option<LlmProviderType>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub model_config: Option<LlmModelConfig>,
    pub prompt_format: Option<LlmPromptFormat>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

/// DTO for API responses (sensitive data hidden)
#[derive(Debug, Serialize)]
pub struct LlmProviderDto {
    pub id: Uuid,
    pub name: String,
    pub provider_type: LlmProviderType,
    pub base_url: Option<String>,
    pub has_api_key: bool,
    pub model_name: String,
    pub model_config: LlmModelConfig,
    pub prompt_format: LlmPromptFormat,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Conversions
impl From<Model> for LlmProviderDomain {
    fn from(entity: Model) -> Self {
        let provider_type = LlmProviderType::from(entity.provider_type);
        
        let model_config: LlmModelConfig = serde_json::from_value(entity.model_config)
            .unwrap_or_default();
            
        let prompt_format: LlmPromptFormat = serde_json::from_str(&entity.prompt_format)
            .unwrap_or(LlmPromptFormat::OpenAI);

        Self {
            id: entity.id,
            name: entity.name,
            provider_type,
            base_url: entity.base_url,
            api_key: entity.api_key,
            model_name: entity.model_name,
            model_config,
            prompt_format,
            enabled: entity.enabled,
            is_default: entity.is_default,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }
}

impl From<LlmProviderDomain> for LlmProviderDto {
    fn from(domain: LlmProviderDomain) -> Self {
        Self {
            id: domain.id,
            name: domain.name,
            provider_type: domain.provider_type,
            base_url: domain.base_url,
            has_api_key: domain.api_key.is_some(),
            model_name: domain.model_name,
            model_config: domain.model_config,
            prompt_format: domain.prompt_format,
            enabled: domain.enabled,
            is_default: domain.is_default,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

impl Default for LlmModelConfig {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            max_tokens: Some(2048),
            top_p: Some(0.9),
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: Some(30),
            custom_parameters: None,
        }
    }
}

impl From<LlmProviderCreateDto> for ActiveModel {
    fn from(dto: LlmProviderCreateDto) -> Self {
        let now = Utc::now();
        let provider_type_str = String::from(dto.provider_type);
        let model_config_json = serde_json::to_value(dto.model_config)
            .unwrap_or(serde_json::to_value(LlmModelConfig::default()).unwrap());
        let prompt_format_str = serde_json::to_string(&dto.prompt_format)
            .unwrap_or("\"OpenAI\"".to_string());

        Self {
            id: Set(Uuid::new_v4()),
            name: Set(dto.name),
            provider_type: Set(provider_type_str),
            base_url: Set(dto.base_url),
            api_key: Set(dto.api_key),
            model_name: Set(dto.model_name),
            model_config: Set(model_config_json),
            prompt_format: Set(prompt_format_str),
            enabled: Set(dto.enabled.unwrap_or(true)),
            is_default: Set(dto.is_default.unwrap_or(false)),
            created_at: Set(now),
            updated_at: Set(now),
        }
    }
}

/// DTOs and enums for request/response
#[derive(Debug, Deserialize)]
pub struct CreateLlmProviderRequest {
    pub name: String,
    pub provider_type: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: String,
    pub model_config: serde_json::Value,
    pub prompt_format: LlmPromptFormat,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub api_endpoint: Option<String>,
    pub encrypted_api_key: Option<String>,
    pub description: Option<String>,
    pub status: Option<LlmProviderStatus>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLlmProviderRequest {
    pub name: Option<String>,
    pub provider_type: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub model_config: Option<serde_json::Value>,
    pub prompt_format: Option<LlmPromptFormat>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub api_endpoint: Option<String>,
    pub encrypted_api_key: Option<String>,
    pub description: Option<String>,
    pub status: Option<LlmProviderStatus>,
}

#[derive(Debug, Deserialize)]
pub struct LlmProviderQueryParams {
    pub name: Option<String>,
    pub provider_type: Option<String>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub status: Option<LlmProviderStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateLlmProviderDto {
    pub name: String,
    pub provider_type: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: String,
    pub model_config: serde_json::Value,
    pub prompt_format: LlmPromptFormat,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub api_endpoint: Option<String>,
    pub encrypted_api_key: Option<String>,
    pub description: Option<String>,
    pub status: Option<LlmProviderStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateLlmProviderDto {
    pub name: Option<String>,
    pub provider_type: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub model_config: Option<serde_json::Value>,
    pub prompt_format: Option<LlmPromptFormat>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub api_endpoint: Option<String>,
    pub encrypted_api_key: Option<String>,
    pub description: Option<String>,
    pub status: Option<LlmProviderStatus>,
}

#[derive(Debug, Serialize)]
pub struct LlmProviderResponseDto {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub base_url: Option<String>,
    pub has_api_key: bool,
    pub model_name: String,
    pub model_config: serde_json::Value,
    pub prompt_format: LlmPromptFormat,
    pub enabled: bool,
    pub is_default: bool,
    pub api_endpoint: Option<String>,
    pub description: Option<String>,
    pub status: LlmProviderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Add alias for backward compatibility
pub type LlmProviderModel = Model;

// Implement From<Model> for LlmProviderResponseDto
impl From<Model> for LlmProviderResponseDto {
    fn from(entity: Model) -> Self {
        let provider_type_str = match entity.provider_type.to_lowercase().as_str() {
            "openai" => "OpenAI".to_string(),
            "ollama" => "Ollama".to_string(),
            "anthropic" => "Anthropic".to_string(),
            "local" => "Local".to_string(),
            "gemini" => "Gemini".to_string(),
            "deepseek" => "DeepSeek".to_string(),
            other => other.to_string(),
        };
        
        // Parse enums from strings, with fallbacks
        let status = LlmProviderStatus::Active; // Default status since it's not in the base model

        let prompt_format = serde_json::from_str::<LlmPromptFormat>(&entity.prompt_format)
            .unwrap_or(LlmPromptFormat::OpenAI);

        // Clone base_url once for use in multiple fields to avoid move issues
        let base_url_cloned = entity.base_url.clone();

        Self {
            id: entity.id,
            name: entity.name,
            provider_type: provider_type_str,
            base_url: base_url_cloned.clone(),
            has_api_key: entity.api_key.is_some(),
            model_name: entity.model_name,
            model_config: entity.model_config,
            prompt_format,
            enabled: entity.enabled,
            is_default: entity.is_default,
            api_endpoint: base_url_cloned,
            description: None,  // Not in base model
            status,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }
}
