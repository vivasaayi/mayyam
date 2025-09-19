use std::sync::Arc;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait, QueryOrder};
use uuid::Uuid;
use chrono::Utc;
use serde_json::Value;
use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit},
    Nonce
};
use rand::{rngs::OsRng, RngCore};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::models::llm_provider::{self, Entity as LlmProvider, Model as LlmProviderModel, ActiveModel as LlmProviderActiveModel, LlmProviderType, LlmProviderStatus, LlmPromptFormat};
use crate::errors::AppError;
use crate::config::Config;

#[derive(Debug)]
pub struct LlmProviderRepository {
    db: Arc<DatabaseConnection>,
    config: Config,
}

impl LlmProviderRepository {
    pub fn new(db: Arc<DatabaseConnection>, config: Config) -> Self {
        Self { db, config }
    }

    pub async fn create(&self, name: String, provider_type: LlmProviderType, model_name: String,
                       api_endpoint: Option<String>, api_key: Option<String>, model_config: Option<Value>,
                       prompt_format: LlmPromptFormat, description: Option<String>, enabled: Option<bool>, is_default: Option<bool>) -> Result<LlmProviderModel, AppError> {
        
        // For now, store api_key directly (in production, you'd want to encrypt it)
        let provider = LlmProviderActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name),
            provider_type: Set(String::from(provider_type)),
            model_name: Set(model_name),
            base_url: Set(api_endpoint), // Map api_endpoint to base_url
            api_key: Set(api_key),
            model_config: Set(model_config.unwrap_or(serde_json::json!({}))),
            prompt_format: Set(serde_json::to_string(&prompt_format).unwrap_or("\"OpenAI\"".to_string())),
            enabled: Set(enabled.unwrap_or(true)), // Default to enabled
            is_default: Set(is_default.unwrap_or(false)), // Default to not default
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let result = provider.insert(&*self.db).await.map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<LlmProviderModel>, AppError> {
        let provider = LlmProvider::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(provider)
    }
    
    pub async fn find_by_name(&self, name: &str) -> Result<Option<LlmProviderModel>, AppError> {
        let provider = LlmProvider::find()
            .filter(llm_provider::Column::Name.eq(name))
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(provider)
    }
    
    pub async fn find_by_model_name(&self, model_name: &str) -> Result<Option<LlmProviderModel>, AppError> {
        let provider = LlmProvider::find()
            .filter(llm_provider::Column::ModelName.eq(model_name))
            .filter(llm_provider::Column::Enabled.eq(true))
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(provider)
    }
    
    pub async fn find_all(&self) -> Result<Vec<LlmProviderModel>, AppError> {
        let providers = LlmProvider::find()
            .order_by_asc(llm_provider::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(providers)
    }

    pub async fn find_active(&self) -> Result<Vec<LlmProviderModel>, AppError> {
        let providers = LlmProvider::find()
            .filter(llm_provider::Column::Enabled.eq(true)) // Use enabled field instead of status
            .order_by_asc(llm_provider::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(providers)
    }

    pub async fn find_by_provider_type(&self, provider_type: LlmProviderType) -> Result<Vec<LlmProviderModel>, AppError> {
        let provider_type_str = String::from(provider_type);
        let providers = LlmProvider::find()
            .filter(llm_provider::Column::ProviderType.eq(provider_type_str))
            .filter(llm_provider::Column::Enabled.eq(true)) // Use enabled field instead of status
            .order_by_asc(llm_provider::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(providers)
    }

    pub async fn update(&self, id: Uuid, name: Option<String>, model_name: Option<String>,
                       api_endpoint: Option<Option<String>>, api_key: Option<Option<String>>,
                       model_config: Option<Option<Value>>, prompt_format: Option<LlmPromptFormat>,
                       description: Option<Option<String>>, status: Option<LlmProviderStatus>,
                       enabled: Option<bool>, is_default: Option<bool>) -> Result<LlmProviderModel, AppError> {
        let provider = LlmProvider::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound("LLM provider not found".to_string()))?;

        let mut active_model: LlmProviderActiveModel = provider.into();
        
        if let Some(name) = name {
            active_model.name = Set(name);
        }
        if let Some(model_name) = model_name {
            active_model.model_name = Set(model_name);
        }
        if let Some(api_endpoint) = api_endpoint {
            active_model.base_url = Set(api_endpoint); // Map api_endpoint to base_url
        }
        if let Some(api_key) = api_key {
            active_model.api_key = Set(api_key); // Store directly for now
        }
        if let Some(model_config) = model_config {
            let json_value = model_config.unwrap_or(serde_json::json!({}));
            active_model.model_config = Set(json_value);
        }
        if let Some(prompt_format) = prompt_format {
            let format_str = serde_json::to_string(&prompt_format).unwrap_or("\"OpenAI\"".to_string());
            active_model.prompt_format = Set(format_str);
        }
        if let Some(enabled) = enabled {
            active_model.enabled = Set(enabled);
        }
        if let Some(is_default) = is_default {
            active_model.is_default = Set(is_default);
        }
        // For status, we'll map it to the enabled field
        if let Some(status) = status {
            let enabled = matches!(status, LlmProviderStatus::Active);
            active_model.enabled = Set(enabled);
        }
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(&*self.db).await.map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        LlmProvider::delete_by_id(id)
            .exec(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(())
    }    pub async fn get_decrypted_api_key(&self, provider: &LlmProviderModel) -> Result<Option<String>, AppError> {
        // For now, return the api_key directly since we're not encrypting it
        Ok(provider.api_key.clone())
    }

    pub async fn test_connection(&self, id: Uuid) -> Result<bool, AppError> {
        let provider = self.find_by_id(id).await?
            .ok_or_else(|| AppError::NotFound("LLM provider not found".to_string()))?;
        
        // Convert string provider_type to enum for matching
        let provider_type = LlmProviderType::from(provider.provider_type);
        
        // TODO: Implement actual connection testing based on provider_type
        // For now, return true as a placeholder
        match provider_type {
            LlmProviderType::OpenAI => {
                // Test OpenAI API connection
                Ok(true)
            },
            LlmProviderType::Ollama => {
                // Test Ollama connection
                Ok(true)
            },
            LlmProviderType::Anthropic => {
                // Test Anthropic API connection
                Ok(true)
            },
            LlmProviderType::Local => {
                // Test local model connection
                Ok(true)
            },
            LlmProviderType::Gemini => {
                // Test Gemini API connection
                Ok(true)
            },
            LlmProviderType::DeepSeek => {
                // Test DeepSeek API connection
                Ok(true)
            },
            LlmProviderType::Custom => {
                // Test custom provider connection
                Ok(true)
            }
        }
    }
}
