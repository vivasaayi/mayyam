use std::sync::Arc;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait};
use uuid::Uuid;
use chrono::Utc;

use crate::models::llm_model::{self, Entity as LlmProviderModelEntity, Model as LlmProviderModel, ActiveModel as LlmProviderModelActiveModel};
use crate::errors::AppError;

#[derive(Debug)]
pub struct LlmProviderModelRepository {
    db: Arc<DatabaseConnection>,
}

impl LlmProviderModelRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self { Self { db } }

    pub async fn list_by_provider(&self, provider_id: Uuid) -> Result<Vec<LlmProviderModel>, AppError> {
        let models = LlmProviderModelEntity::find()
            .filter(llm_model::Column::ProviderId.eq(provider_id))
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        Ok(models)
    }

    pub async fn create(&self, provider_id: Uuid, model_name: String, model_config: serde_json::Value, enabled: bool) -> Result<LlmProviderModel, AppError> {
        let model = LlmProviderModelActiveModel {
            id: Set(Uuid::new_v4()),
            provider_id: Set(provider_id),
            model_name: Set(model_name),
            model_config: Set(model_config),
            enabled: Set(enabled),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };
        let result = model.insert(&*self.db).await.map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn update(&self, id: Uuid, model_name: Option<String>, model_config: Option<serde_json::Value>, enabled: Option<bool>) -> Result<LlmProviderModel, AppError> {
        let existing = LlmProviderModelEntity::find_by_id(id).one(&*self.db).await.map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound("Model not found".to_string()))?;
        let mut active: LlmProviderModelActiveModel = existing.into();
        if let Some(n) = model_name { active.model_name = Set(n); }
        if let Some(c) = model_config { active.model_config = Set(c); }
        if let Some(e) = enabled { active.enabled = Set(e); }
        active.updated_at = Set(Utc::now());
        let result = active.update(&*self.db).await.map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        LlmProviderModelEntity::delete_by_id(id).exec(&*self.db).await.map_err(AppError::from)?;
        Ok(())
    }

    pub async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<LlmProviderModel, AppError> {
        self.update(id, None, None, Some(enabled)).await
    }
}
