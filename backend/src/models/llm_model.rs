use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
// no ActiveModel construction here

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "llm_provider_models")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub provider_id: Uuid,
    pub model_name: String,
    pub model_config: Json,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmProviderModelDto {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub model_name: String,
    pub model_config: serde_json::Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Model> for LlmProviderModelDto {
    fn from(entity: Model) -> Self {
        Self {
            id: entity.id,
            provider_id: entity.provider_id,
            model_name: entity.model_name,
            model_config: entity.model_config,
            enabled: entity.enabled,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }
}
