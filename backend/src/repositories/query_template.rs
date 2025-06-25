use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, Set, QueryFilter, ColumnTrait, QueryOrder};
use sea_orm::entity::*;
use sea_orm::{DbErr, ActiveValue::NotSet};
use uuid::Uuid;
use std::sync::Arc;
use chrono::Utc;

use crate::config::Config;
use crate::models::query_template::{self, Model, ActiveModel};
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
        query_template::Entity::find_by_id(id)
            .one(&*self.db)
            .await
    }

    // Create a new template
    pub async fn create(&self, template: &CreateQueryTemplateRequest, user_id: Uuid) -> Result<Model, DbErr> {
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

        new_template.insert(&*self.db).await
    }

    // Update an existing template
    pub async fn update(&self, id: Uuid, template: &UpdateQueryTemplateRequest) -> Result<Model, DbErr> {
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
                template_active.connection_type = Set(connection_type.clone());
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
            Err(DbErr::RecordNotFound(
                format!("Template with ID {} not found", id)
            ))
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
            Err(DbErr::RecordNotFound(
                format!("Template with ID {} not found", id)
            ))
        }
    }
}
