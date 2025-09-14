use std::sync::Arc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, IntoActiveModel, ModelTrait, Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Set
};
use chrono::Utc;
use uuid::Uuid;
use tracing::{info, error};

use crate::models::aws_resource::{
    self, ActiveModel, AwsResourceDto, AwsResourceQuery, AwsResourcePage, Entity as AwsResource, Model,
};
use crate::errors::AppError;
use crate::config::Config;

#[derive(Debug)]
pub struct AwsResourceRepository {
    db: Arc<DatabaseConnection>,
    config: Config,
}

impl AwsResourceRepository {
    pub fn new(db: Arc<DatabaseConnection>, config: Config) -> Self {
        Self { db, config }
    }

    // Create a new AWS resource entry
    pub async fn create(&self, resource: &AwsResourceDto) -> Result<Model, AppError> {
        let now = Utc::now();
        
        let active_model = ActiveModel {
            id: Set(Uuid::new_v4()),
            account_id: Set(resource.account_id.clone()),
            profile: Set(resource.profile.clone()),
            region: Set(resource.region.clone()),
            resource_type: Set(resource.resource_type.clone()),
            resource_id: Set(resource.resource_id.clone()),
            arn: Set(resource.arn.clone()),
            name: Set(resource.name.clone()),
            tags: Set(resource.tags.clone()),
            resource_data: Set(resource.resource_data.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            last_refreshed: Set(now),
        };

        let model = active_model
            .insert(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        info!("AWS resource created: {}", model.id);
        Ok(model)
    }

    // Update an existing AWS resource
    pub async fn update(&self, id: Uuid, resource: &AwsResourceDto) -> Result<Model, AppError> {
        let aws_resource = AwsResource::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?
            .ok_or_else(|| AppError::NotFound(format!("AWS resource with ID {} not found", id)))?;

        let now = Utc::now();
        let mut active_model = aws_resource.into_active_model();

        active_model.account_id = Set(resource.account_id.clone());
        active_model.profile = Set(resource.profile.clone());
        active_model.region = Set(resource.region.clone());
        active_model.resource_type = Set(resource.resource_type.clone());
        active_model.resource_id = Set(resource.resource_id.clone());
        active_model.arn = Set(resource.arn.clone());
        active_model.name = Set(resource.name.clone());
        active_model.tags = Set(resource.tags.clone());
        active_model.resource_data = Set(resource.resource_data.clone());
        active_model.updated_at = Set(now);
        active_model.last_refreshed = Set(now);

        let updated_model = active_model
            .update(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        info!("AWS resource updated: {}", updated_model.id);
        Ok(updated_model)
    }

    // Find resource by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Model>, AppError> {
        let resource = AwsResource::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(resource)
    }

    // Find resource by ARN
    pub async fn find_by_arn(&self, arn: &str) -> Result<Option<Model>, AppError> {
        let resource = AwsResource::find()
            .filter(aws_resource::Column::Arn.eq(arn))
            .one(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(resource)
    }

    // Find resources by account ID and type
    pub async fn find_by_account_and_type(
        &self,
        account_id: &str,
        resource_type: &str,
    ) -> Result<Vec<Model>, AppError> {
        let resources = AwsResource::find()
            .filter(
                Condition::all()
                    .add(aws_resource::Column::AccountId.eq(account_id))
                    .add(aws_resource::Column::ResourceType.eq(resource_type)),
            )
            .order_by(aws_resource::Column::Name, Order::Asc)
            .all(&*self.db)
            .await
            .map_err(|e| {
                AppError::Database(e)
            })?;

        Ok(resources)
    }

    // Search resources with pagination
    pub async fn search(
        &self,
        query: &AwsResourceQuery,
    ) -> Result<AwsResourcePage, AppError> {
        let page = query.page.unwrap_or(0);
        let page_size = query.page_size.unwrap_or(10);
        
        let mut condition = Condition::all();
        
        if let Some(account_id) = &query.account_id {
            condition = condition.add(aws_resource::Column::AccountId.eq(account_id.clone()));
        }
        
        if let Some(profile) = &query.profile {
            condition = condition.add(aws_resource::Column::Profile.eq(profile.clone()));
        }
        
        if let Some(region) = &query.region {
            condition = condition.add(aws_resource::Column::Region.eq(region.clone()));
        }
        
        if let Some(resource_type) = &query.resource_type {
            condition = condition.add(aws_resource::Column::ResourceType.eq(resource_type.clone()));
        }
        
        if let Some(resource_id) = &query.resource_id {
            condition = condition.add(aws_resource::Column::ResourceId.eq(resource_id.clone()));
        }
        
        if let Some(name) = &query.name {
            condition = condition.add(aws_resource::Column::Name.like(format!("%{}%", name)));
        }
        
        // Count total results first
        let total = AwsResource::find()
            .filter(condition.clone())
            .count(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;
        
        // Then fetch the requested page
        let resources = AwsResource::find()
            .filter(condition)
            .order_by(aws_resource::Column::UpdatedAt, Order::Desc)
            .limit(Some(page_size))
            .offset(Some(page * page_size))
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;
        
        let total_pages = (total as f64 / page_size as f64).ceil() as u64;
        
        Ok(AwsResourcePage {
            resources,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    // Delete a resource
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let res = AwsResource::delete_by_id(id)
            .exec(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        if res.rows_affected == 0 {
            return Err(AppError::NotFound(format!(
                "AWS resource with ID {} not found",
                id
            )));
        }

        info!("AWS resource deleted: {}", id);
        Ok(())
    }

    // Update resource data only
    pub async fn update_resource_data(
        &self,
        id: Uuid,
        resource_data: serde_json::Value,
    ) -> Result<Model, AppError> {
        let aws_resource = AwsResource::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?
            .ok_or_else(|| AppError::NotFound(format!("AWS resource with ID {} not found", id)))?;

        let now = Utc::now();
        let mut active_model = aws_resource.into_active_model();

        active_model.resource_data = Set(resource_data);
        active_model.updated_at = Set(now);
        active_model.last_refreshed = Set(now);

        let updated_model = active_model
            .update(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        info!("AWS resource data updated: {}", updated_model.id);
        Ok(updated_model)
    }
}