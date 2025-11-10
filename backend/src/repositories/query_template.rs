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


use chrono::Utc;
use sea_orm::entity::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use sea_orm::{ActiveValue::NotSet, DbErr};
use std::sync::Arc;
use uuid::Uuid;

use crate::config::Config;
use crate::models::query_template::{self, ActiveModel, Model};
use crate::models::query_template::{CreateQueryTemplateRequest, UpdateQueryTemplateRequest};

pub struct QueryTemplateRepository {
    db: Arc<DatabaseConnection>,
    config: Config,
}

impl QueryTemplateRepository {
    pub fn new(db: Arc<DatabaseConnection>, config: Config) -> Self {
        Self { db, config }
    }

    // Find all query templates
    pub async fn find_all(&self) -> Result<Vec<Model>, DbErr> {
        query_template::Entity::find()
            .order_by(query_template::Column::DisplayOrder, sea_orm::Order::Asc)
            .all(&*self.db)
            .await
    }

    // Find templates by connection type
    pub async fn find_by_connection_type(&self, conn_type: &str) -> Result<Vec<Model>, DbErr> {
        query_template::Entity::find()
            .filter(query_template::Column::ConnectionType.eq(conn_type))
            .order_by(query_template::Column::DisplayOrder, sea_orm::Order::Asc)
            .all(&*self.db)
            .await
    }

    // Find template by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Model>, DbErr> {
        query_template::Entity::find_by_id(id).one(&*self.db).await
    }

    // Create a new template
    pub async fn create(
        &self,
        template: &CreateQueryTemplateRequest,
        user_id: Uuid,
    ) -> Result<Model, DbErr> {
        let now = Utc::now();

        let new_template = ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(template.name.clone()),
            query: Set(template.query.clone()),
            description: Set(template.description.clone()),
            connection_type: Set(template.connection_type.clone()),
            category: Set(template.category.clone()),
            is_favorite: Set(template.is_favorite.unwrap_or(false)),
            display_order: Set(template.display_order.unwrap_or(999)),
            created_by: Set(user_id),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = new_template.insert(&*self.db).await?;
        Ok(result)
    }

    // Update an existing template
    pub async fn update(
        &self,
        id: Uuid,
        template: &UpdateQueryTemplateRequest,
    ) -> Result<Model, DbErr> {
        let template_result = query_template::Entity::find_by_id(id)
            .one(&*self.db)
            .await?;

        if let Some(template_model) = template_result {
            let mut template_active: ActiveModel = template_model.into();

            if let Some(name) = &template.name {
                template_active.name = Set(name.clone());
            }

            if let Some(query) = &template.query {
                template_active.query = Set(query.clone());
            }

            if let Some(description) = &template.description {
                template_active.description = Set(Some(description.clone()));
            }

            if let Some(connection_type) = &template.connection_type {
                template_active.connection_type = Set(Some(connection_type.clone()));
            } else {
                template_active.connection_type = Set(None);
            }

            if let Some(category) = &template.category {
                template_active.category = Set(Some(category.clone()));
            }

            if let Some(is_favorite) = template.is_favorite {
                template_active.is_favorite = Set(is_favorite);
            }

            if let Some(display_order) = template.display_order {
                template_active.display_order = Set(display_order);
            }

            template_active.updated_at = Set(Utc::now());

            template_active.update(&*self.db).await
        } else {
            Err(DbErr::RecordNotFound(format!(
                "Template with ID {} not found",
                id
            )))
        }
    }

    // Delete a template
    pub async fn delete(&self, id: Uuid) -> Result<(), DbErr> {
        let template_result = query_template::Entity::find_by_id(id)
            .one(&*self.db)
            .await?;

        if let Some(template_model) = template_result {
            let template_active: ActiveModel = template_model.into();
            template_active.delete(&*self.db).await?;
            Ok(())
        } else {
            Err(DbErr::RecordNotFound(format!(
                "Template with ID {} not found",
                id
            )))
        }
    }

    // Find common templates (connection_type is NULL)
    pub async fn find_common_templates(&self) -> Result<Vec<Model>, DbErr> {
        query_template::Entity::find()
            .filter(query_template::Column::ConnectionType.is_null())
            .order_by(query_template::Column::DisplayOrder, sea_orm::Order::Asc)
            .all(&*self.db)
            .await
    }

    // Find templates for a specific connection type, including common templates
    pub async fn find_by_connection_type_with_common(
        &self,
        conn_type: &str,
    ) -> Result<Vec<Model>, DbErr> {
        // Get specific connection type templates
        let specific_templates = self.find_by_connection_type(conn_type).await?;

        // Get common templates
        let common_templates = self.find_common_templates().await?;

        // Combine both lists
        let mut all_templates = Vec::new();
        all_templates.extend(common_templates);
        all_templates.extend(specific_templates);

        // Sort by display_order
        all_templates.sort_by(|a, b| a.display_order.cmp(&b.display_order));

        Ok(all_templates)
    }
}
