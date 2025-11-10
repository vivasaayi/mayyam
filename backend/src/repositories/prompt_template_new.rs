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


use std::sync::Arc;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait, QueryOrder, QuerySelect};
use uuid::Uuid;
use chrono::Utc;
use serde_json::Value;

use crate::models::prompt_template::{self, Entity as PromptTemplate, Model as PromptTemplateModel, ActiveModel as PromptTemplateActiveModel, PromptCategory};
use crate::errors::AppError;

pub struct PromptTemplateRepository {
    db: Arc<DatabaseConnection>,
}

impl PromptTemplateRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, name: String, category: PromptCategory,
                       template_content: String, variables: Option<Value>,
                       tags: Option<Vec<String>>, is_system: bool, created_by: Option<Uuid>) -> Result<PromptTemplateModel, AppError> {
        let new_prompt = PromptTemplateActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name),
            category: Set(category.to_string()),
            resource_type: Set(None),
            workflow_type: Set(None),
            prompt_template: Set(template_content),
            variables: Set(variables.unwrap_or_else(|| serde_json::json!({}))),
            version: Set("1".to_string()),
            is_active: Set(true),
            is_system: Set(is_system),
            description: Set(None),
            tags: Set(tags.map(|t| serde_json::json!(t)).unwrap_or_else(|| serde_json::json!([]))),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            created_by: Set(created_by),
        };

        let result = new_prompt.insert(&*self.db).await.map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<PromptTemplateModel>, AppError> {
        let prompt = PromptTemplate::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompt)
    }
    
    pub async fn find_by_name(&self, name: &str) -> Result<Option<PromptTemplateModel>, AppError> {
        let prompt = PromptTemplate::find()
            .filter(prompt_template::Column::Name.eq(name))
            .filter(prompt_template::Column::IsActive.eq(true))
            .order_by_desc(prompt_template::Column::Version)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompt)
    }
    
    pub async fn find_all(&self) -> Result<Vec<PromptTemplateModel>, AppError> {
        let prompts = PromptTemplate::find()
            .filter(prompt_template::Column::IsActive.eq(true))
            .order_by_asc(prompt_template::Column::Name)
            .order_by_desc(prompt_template::Column::Version)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompts)
    }

    pub async fn find_by_category(&self, category: PromptCategory) -> Result<Vec<PromptTemplateModel>, AppError> {
        let prompts = PromptTemplate::find()
            .filter(prompt_template::Column::Category.eq(category.to_string()))
            .filter(prompt_template::Column::IsActive.eq(true))
            .order_by_asc(prompt_template::Column::Name)
            .order_by_desc(prompt_template::Column::Version)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompts)
    }

    pub async fn find_by_resource_type(&self, resource_type: &str) -> Result<Vec<PromptTemplateModel>, AppError> {
        let prompts = PromptTemplate::find()
            .filter(prompt_template::Column::ResourceType.eq(Some(resource_type.to_string())))
            .filter(prompt_template::Column::IsActive.eq(true))
            .order_by_asc(prompt_template::Column::Name)
            .order_by_desc(prompt_template::Column::Version)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompts)
    }

    pub async fn find_by_tags(&self, _tags: &[String]) -> Result<Vec<PromptTemplateModel>, AppError> {
        // Simplified implementation - in real scenario would need proper JSON querying
        let prompts = PromptTemplate::find()
            .filter(prompt_template::Column::IsActive.eq(true))
            .order_by_asc(prompt_template::Column::Name)
            .order_by_desc(prompt_template::Column::Version)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompts)
    }

    pub async fn find_system_prompts(&self) -> Result<Vec<PromptTemplateModel>, AppError> {
        let prompts = PromptTemplate::find()
            .filter(prompt_template::Column::IsSystem.eq(true))
            .filter(prompt_template::Column::IsActive.eq(true))
            .order_by_asc(prompt_template::Column::Name)
            .order_by_desc(prompt_template::Column::Version)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompts)
    }

    pub async fn find_versions(&self, name: &str) -> Result<Vec<PromptTemplateModel>, AppError> {
        let prompts = PromptTemplate::find()
            .filter(prompt_template::Column::Name.eq(name))
            .order_by_desc(prompt_template::Column::Version)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompts)
    }

    pub async fn create_version(&self, parent_id: Uuid, template_content: String, 
                               variables: Option<Value>, description: Option<String>) -> Result<PromptTemplateModel, AppError> {
        let parent = self.find_by_id(parent_id).await?
            .ok_or_else(|| AppError::NotFound("Parent prompt template not found".to_string()))?;

        // Get the next version number - parse current version and increment
        let latest_version = PromptTemplate::find()
            .filter(prompt_template::Column::Name.eq(&parent.name))
            .order_by_desc(prompt_template::Column::Version)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?
            .map(|p| p.version.parse::<i32>().unwrap_or(0))
            .unwrap_or(0);

        let new_prompt = PromptTemplateActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(parent.name.clone()),
            description: Set(description.or(parent.description)),
            category: Set(parent.category),
            resource_type: Set(parent.resource_type),
            workflow_type: Set(parent.workflow_type),
            prompt_template: Set(template_content),
            variables: Set(variables.unwrap_or_else(|| serde_json::json!({}))),
            version: Set((latest_version + 1).to_string()),
            is_active: Set(true),
            is_system: Set(parent.is_system),
            tags: Set(parent.tags),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            created_by: Set(parent.created_by),
        };

        let result = new_prompt.insert(&*self.db).await.map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn update(&self, id: Uuid, description: Option<Option<String>>, 
                       template_content: Option<String>, variables: Option<Option<Value>>,
                       tags: Option<Option<Vec<String>>>) -> Result<PromptTemplateModel, AppError> {
        let prompt = PromptTemplate::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound("Prompt template not found".to_string()))?;

        let mut active_model: PromptTemplateActiveModel = prompt.into();
        
        if let Some(description) = description {
            active_model.description = Set(description);
        }
        if let Some(template_content) = template_content {
            active_model.prompt_template = Set(template_content);
        }
        if let Some(variables) = variables {
            active_model.variables = Set(variables.unwrap_or_else(|| serde_json::json!({})));
        }
        if let Some(tags) = tags {
            active_model.tags = Set(tags.map(|t| serde_json::json!(t)).unwrap_or_else(|| serde_json::json!([])));
        }
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(&*self.db).await.map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        // Soft delete by setting is_active to false
        let prompt = PromptTemplate::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound("Prompt template not found".to_string()))?;

        let mut active_model: PromptTemplateActiveModel = prompt.into();
        active_model.is_active = Set(false);
        active_model.updated_at = Set(Utc::now());

        active_model.update(&*self.db).await.map_err(AppError::from)?;
        Ok(())
    }

    pub async fn search(&self, query: &str) -> Result<Vec<PromptTemplateModel>, AppError> {
        let prompts = PromptTemplate::find()
            .filter(
                prompt_template::Column::Name.contains(query)
                    .or(prompt_template::Column::Description.contains(query))
                    .or(prompt_template::Column::PromptTemplate.contains(query))
            )
            .filter(prompt_template::Column::IsActive.eq(true))
            .order_by_asc(prompt_template::Column::Name)
            .order_by_desc(prompt_template::Column::Version)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(prompts)
    }
}
