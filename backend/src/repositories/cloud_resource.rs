use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, Order,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::cloud_resource::{
    self, ActiveModel, CloudResourceDto, CloudResourcePage, CloudResourceQuery,
    Entity as CloudResource, Model,
};

#[derive(Debug)]
pub struct CloudResourceRepository {
    db: Arc<DatabaseConnection>,
}

impl CloudResourceRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, dto: &CloudResourceDto) -> Result<Model, AppError> {
        let now = Utc::now();
        let active = ActiveModel {
            id: Set(Uuid::new_v4()),
            sync_id: Set(dto.sync_id),
            provider: Set(dto.provider.clone()),
            account_id: Set(dto.account_id.clone()),
            region: Set(dto.region.clone()),
            resource_type: Set(dto.resource_type.clone()),
            resource_id: Set(dto.resource_id.clone()),
            arn_or_uri: Set(dto.arn_or_uri.clone()),
            name: Set(dto.name.clone()),
            tags: Set(dto.tags.clone()),
            resource_data: Set(dto.resource_data.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            last_refreshed: Set(now),
        };

        let model = active.insert(&*self.db).await.map_err(AppError::Database)?;
        Ok(model)
    }

    pub async fn search(&self, query: &CloudResourceQuery) -> Result<CloudResourcePage, AppError> {
        let page = query.page.unwrap_or(0);
        let page_size = query.page_size.unwrap_or(10);

        let mut cond = Condition::all();
        if let Some(sync_id) = query.sync_id {
            cond = cond.add(cloud_resource::Column::SyncId.eq(sync_id));
        }
        if let Some(provider) = &query.provider {
            cond = cond.add(cloud_resource::Column::Provider.eq(provider.clone()));
        }
        if let Some(account_id) = &query.account_id {
            cond = cond.add(cloud_resource::Column::AccountId.eq(account_id.clone()));
        }
        if let Some(region) = &query.region {
            cond = cond.add(cloud_resource::Column::Region.eq(region.clone()));
        }
        if let Some(resource_type) = &query.resource_type {
            cond = cond.add(cloud_resource::Column::ResourceType.eq(resource_type.clone()));
        }
        if let Some(resource_id) = &query.resource_id {
            cond = cond.add(cloud_resource::Column::ResourceId.eq(resource_id.clone()));
        }
        if let Some(name) = &query.name {
            cond = cond.add(cloud_resource::Column::Name.like(format!("%{}%", name)));
        }

        let total = CloudResource::find()
            .filter(cond.clone())
            .count(&*self.db)
            .await
            .map_err(AppError::Database)?;
        let paginator = CloudResource::find()
            .filter(cond)
            .order_by(cloud_resource::Column::UpdatedAt, Order::Desc)
            .paginate(&*self.db, page_size);

        let rows = paginator
            .fetch_page(page)
            .await
            .map_err(AppError::Database)?;
        let total_pages = (total as f64 / page_size as f64).ceil() as u64;
        Ok(CloudResourcePage {
            resources: rows,
            total,
            page,
            page_size,
            total_pages,
        })
    }
}
