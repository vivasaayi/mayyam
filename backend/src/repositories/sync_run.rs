use chrono::Utc;
use sea_orm::{
    prelude::*, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::sync_run::{
    self, ActiveModel, Entity as SyncRun, Model, SyncRunCreateDto, SyncRunDto, SyncRunQueryParams,
};

#[derive(Debug)]
pub struct SyncRunRepository {
    db: Arc<DatabaseConnection>,
}

impl SyncRunRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, dto: SyncRunCreateDto) -> Result<SyncRunDto, AppError> {
        debug!("Creating sync_run: {:?}", dto);

        let now = Utc::now();
        let id = Uuid::new_v4();

        // Merge regions/all_regions into metadata for retrieval during sync
        let mut metadata = dto.metadata.unwrap_or_else(|| serde_json::json!({}));
        if let Some(true) = dto.all_regions {
            metadata["all_regions"] = serde_json::json!(true);
        }
        if let Some(regions) = dto.regions.clone() {
            metadata["regions"] = serde_json::json!(regions);
        }
        let model = ActiveModel {
            id: Set(id),
            name: Set(dto.name),
            aws_account_id: Set(dto.aws_account_id),
            account_id: Set(dto.account_id),
            profile: Set(dto.profile),
            region: Set(dto.region),
            status: Set("created".to_string()),
            total_resources: Set(0),
            success_count: Set(0),
            failure_count: Set(0),
            error_summary: Set(None),
            metadata: Set(metadata),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let inserted: Model = model.insert(&*self.db).await.map_err(AppError::Database)?;

        debug!("Created sync_run: {:?}", inserted);

        Ok(SyncRunDto::from(inserted))
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<SyncRunDto>, AppError> {
        let res = SyncRun::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::Database)?;
        Ok(res.map(|m| m.into()))
    }

    pub async fn list(&self, q: SyncRunQueryParams) -> Result<Vec<SyncRunDto>, AppError> {
        let mut cond = Condition::all();
        if let Some(status) = q.status {
            cond = cond.add(sync_run::Column::Status.eq(status));
        }
        let limit = q.limit.unwrap_or(50);
        let offset = q.offset.unwrap_or(0);
        let rows = SyncRun::find()
            .filter(cond)
            .order_by_desc(sync_run::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(AppError::Database)?;
        Ok(rows.into_iter().map(|m| m.into()).collect())
    }

    pub async fn mark_running(&self, id: Uuid) -> Result<(), AppError> {
        if let Some(m) = SyncRun::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::Database)?
        {
            let mut am: ActiveModel = m.into();
            am.status = Set("running".to_string());
            am.started_at = Set(Some(Utc::now()));
            am.update(&*self.db).await.map_err(AppError::Database)?;
        }
        Ok(())
    }

    pub async fn complete(
        &self,
        id: Uuid,
        total: i32,
        success: i32,
        failure: i32,
    ) -> Result<(), AppError> {
        if let Some(m) = SyncRun::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::Database)?
        {
            let mut am: ActiveModel = m.into();
            am.status = Set("completed".to_string());
            am.total_resources = Set(total);
            am.success_count = Set(success);
            am.failure_count = Set(failure);
            am.completed_at = Set(Some(Utc::now()));
            am.update(&*self.db).await.map_err(AppError::Database)?;
        }
        Ok(())
    }

    pub async fn update_metadata(
        &self,
        id: Uuid,
        patch: serde_json::Value,
    ) -> Result<(), AppError> {
        if let Some(m) = SyncRun::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::Database)?
        {
            let mut meta = m.metadata.clone();
            // Merge simple keys from patch into existing metadata
            if let Some(obj) = patch.as_object() {
                for (k, v) in obj.iter() {
                    meta[k] = v.clone();
                }
            }
            let mut am: ActiveModel = m.into();
            am.metadata = Set(meta);
            am.updated_at = Set(Utc::now());
            am.update(&*self.db).await.map_err(AppError::Database)?;
        }
        Ok(())
    }

    pub async fn fail(&self, id: Uuid, error_summary: String) -> Result<(), AppError> {
        if let Some(m) = SyncRun::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::Database)?
        {
            let mut am: ActiveModel = m.into();
            am.status = Set("failed".to_string());
            am.error_summary = Set(Some(error_summary));
            am.completed_at = Set(Some(Utc::now()));
            am.update(&*self.db).await.map_err(AppError::Database)?;
        }
        Ok(())
    }
}
