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


use crate::errors::AppError;
use crate::models::prompt_template::{ActiveModel, Column, Entity, Model};
use crate::models::prompt_template::{CreatePromptTemplateDto, UpdatePromptTemplateDto};
use sea_orm::*;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug)]
pub struct PromptTemplateRepository {
    db: DatabaseConnection,
}

impl PromptTemplateRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create(&self, dto: CreatePromptTemplateDto) -> Result<Model, AppError> {
        let active_model = ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(dto.name),
            category: Set(dto.category.to_string()),
            resource_type: Set(dto.resource_type),
            workflow_type: Set(dto.workflow_type),
            prompt_template: Set(dto.prompt_template),
            variables: Set(
                serde_json::to_value(dto.variables).unwrap_or(serde_json::Value::Array(vec![]))
            ),
            version: Set(dto.version.unwrap_or("1.0".to_string())),
            is_active: Set(dto.is_active.unwrap_or(true)),
            is_system: Set(dto.is_system.unwrap_or(false)),
            description: Set(dto.description),
            tags: Set(serde_json::to_value(dto.tags).unwrap_or(serde_json::Value::Array(vec![]))),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            created_by: Set(None), // Will be set by authentication middleware
        };

        let result = Entity::insert(active_model)
            .exec(&self.db)
            .await
            .map_err(|e| {
                error!("Failed to create prompt template: {}", e);
                AppError::Database(e)
            })?;

        self.find_by_id(&result.last_insert_id).await
    }

    pub async fn find_by_id(&self, id: &Uuid) -> Result<Model, AppError> {
        Entity::find_by_id(*id)
            .one(&self.db)
            .await
            .map_err(|e| {
                error!("Failed to find prompt template by id: {}", e);
                AppError::Database(e)
            })?
            .ok_or_else(|| AppError::NotFound(format!("Prompt template with id {} not found", id)))
    }

    pub async fn find_all(&self) -> Result<Vec<Model>, AppError> {
        Entity::find().all(&self.db).await.map_err(|e| {
            error!("Failed to find all prompt templates: {}", e);
            AppError::Database(e)
        })
    }

    pub async fn find_by_category(&self, category: &str) -> Result<Vec<Model>, AppError> {
        Entity::find()
            .filter(Column::Category.eq(category))
            .all(&self.db)
            .await
            .map_err(|e| {
                error!("Failed to find prompt templates by category: {}", e);
                AppError::Database(e)
            })
    }

    pub async fn find_system_templates(&self) -> Result<Vec<Model>, AppError> {
        Entity::find()
            .filter(Column::IsSystem.eq(true))
            .all(&self.db)
            .await
            .map_err(|e| {
                error!("Failed to find system prompt templates: {}", e);
                AppError::Database(e)
            })
    }

    pub async fn find_system_prompts(&self) -> Result<Vec<Model>, AppError> {
        self.find_system_templates().await
    }

    pub async fn update(&self, id: &Uuid, dto: UpdatePromptTemplateDto) -> Result<Model, AppError> {
        let mut active_model: ActiveModel = Entity::find_by_id(*id)
            .one(&self.db)
            .await
            .map_err(|e| {
                error!("Failed to find prompt template for update: {}", e);
                AppError::Database(e)
            })?
            .ok_or_else(|| AppError::NotFound(format!("Prompt template with id {} not found", id)))?
            .into();

        if let Some(name) = dto.name {
            active_model.name = Set(name);
        }
        if let Some(description) = dto.description {
            active_model.description = Set(Some(description));
        }
        if let Some(category) = dto.category {
            active_model.category = Set(category.to_string());
        }
        if let Some(resource_type) = dto.resource_type {
            active_model.resource_type = Set(Some(resource_type));
        }
        if let Some(workflow_type) = dto.workflow_type {
            active_model.workflow_type = Set(Some(workflow_type));
        }
        if let Some(prompt_template) = dto.prompt_template {
            active_model.prompt_template = Set(prompt_template);
        }
        if let Some(variables) = dto.variables {
            active_model.variables =
                Set(serde_json::to_value(variables).unwrap_or(serde_json::Value::Array(vec![])));
        }
        if let Some(version) = dto.version {
            active_model.version = Set(version);
        }
        if let Some(is_active) = dto.is_active {
            active_model.is_active = Set(is_active);
        }
        if let Some(is_system) = dto.is_system {
            active_model.is_system = Set(is_system);
        }
        if let Some(tags) = dto.tags {
            active_model.tags =
                Set(serde_json::to_value(tags).unwrap_or(serde_json::Value::Array(vec![])));
        }

        active_model.updated_at = Set(chrono::Utc::now());

        let result = active_model.update(&self.db).await.map_err(|e| {
            error!("Failed to update prompt template: {}", e);
            AppError::Database(e)
        })?;

        Ok(result)
    }

    pub async fn delete(&self, id: &Uuid) -> Result<(), AppError> {
        let result = Entity::delete_by_id(*id)
            .exec(&self.db)
            .await
            .map_err(|e| {
                error!("Failed to delete prompt template: {}", e);
                AppError::Database(e)
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!(
                "Prompt template with id {} not found",
                id
            )));
        }

        info!("Deleted prompt template with id: {}", id);
        Ok(())
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Model>, AppError> {
        Entity::find()
            .filter(
                Condition::any()
                    .add(Column::Name.contains(query))
                    .add(Column::Description.contains(query))
                    .add(Column::PromptTemplate.contains(query)), // Use correct field name
            )
            .all(&self.db)
            .await
            .map_err(|e| {
                error!("Failed to search prompt templates: {}", e);
                AppError::Database(e)
            })
    }
}
