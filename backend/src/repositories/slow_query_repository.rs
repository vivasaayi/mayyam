use crate::models::slow_query_event::{SlowQueryEvent, Entity as SlowQueryEntity, Column as SlowQueryColumn};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, PaginatorTrait, QueryOrder, IntoActiveModel, QuerySelect};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDateTime, Duration};

#[derive(Clone)]
pub struct SlowQueryRepository {
    db: Arc<DatabaseConnection>,
}

impl SlowQueryRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, event: SlowQueryEvent) -> Result<SlowQueryEvent, String> {
        let active_model: crate::models::slow_query_event::ActiveModel = event.into();
        active_model.insert(&*self.db)
            .await
            .map_err(|e| format!("Failed to create slow query event: {}", e))
    }

    pub async fn create_many(&self, events: Vec<SlowQueryEvent>) -> Result<(), String> {
        let active_models: Vec<crate::models::slow_query_event::ActiveModel> =
            events.into_iter().map(|e| e.into()).collect();

        SlowQueryEntity::insert_many(active_models)
            .exec(&*self.db)
            .await
            .map_err(|e| format!("Failed to create slow query events: {}", e))?;
        Ok(())
    }

    pub async fn find_by_cluster_and_time_range(
        &self,
        cluster_id: Uuid,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Vec<SlowQueryEvent>, String> {
        SlowQueryEntity::find()
            .filter(SlowQueryColumn::ClusterId.eq(cluster_id))
            .filter(SlowQueryColumn::EventTimestamp.gte(start_time))
            .filter(SlowQueryColumn::EventTimestamp.lte(end_time))
            .order_by_desc(SlowQueryColumn::EventTimestamp)
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to find slow query events: {}", e))
    }

    pub async fn find_top_by_total_time(
        &self,
        cluster_id: Option<Uuid>,
        hours: i64,
        limit: u64,
    ) -> Result<Vec<SlowQueryEvent>, String> {
        let cutoff_time = chrono::Utc::now().naive_utc() - Duration::hours(hours);

        let mut query = SlowQueryEntity::find()
            .filter(SlowQueryColumn::EventTimestamp.gte(cutoff_time));

        if let Some(cluster) = cluster_id {
            query = query.filter(SlowQueryColumn::ClusterId.eq(cluster));
        }

        query
            .order_by_desc(SlowQueryColumn::QueryTime)
            .limit(limit)
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to find top slow queries: {}", e))
    }

    pub async fn update_fingerprint(&self, event_id: Uuid, fingerprint_id: Uuid) -> Result<(), String> {
        let mut active_model = SlowQueryEntity::find_by_id(event_id)
            .one(&*self.db)
            .await
            .map_err(|e| format!("Failed to find slow query event: {}", e))?
            .ok_or_else(|| "Slow query event not found".to_string())?
            .into_active_model();

        active_model.fingerprint_id = Set(Some(fingerprint_id));
        active_model.update(&*self.db)
            .await
            .map_err(|e| format!("Failed to update fingerprint: {}", e))?;
        Ok(())
    }

    pub async fn count_by_cluster(&self, cluster_id: Uuid) -> Result<u64, String> {
        SlowQueryEntity::find()
            .filter(SlowQueryColumn::ClusterId.eq(cluster_id))
            .count(&*self.db)
            .await
            .map_err(|e| format!("Failed to count slow query events: {}", e))
    }

    pub async fn delete_old_events(&self, days_to_keep: i64) -> Result<u64, String> {
        let cutoff_date = chrono::Utc::now().naive_utc() - Duration::days(days_to_keep);

        let delete_result = SlowQueryEntity::delete_many()
            .filter(SlowQueryColumn::EventTimestamp.lt(cutoff_date))
            .exec(&*self.db)
            .await
            .map_err(|e| format!("Failed to delete old events: {}", e))?;

        Ok(delete_result.rows_affected)
    }

    pub async fn find_by_fingerprint(&self, fingerprint_id: Uuid, limit: u64) -> Result<Vec<SlowQueryEvent>, String> {
        SlowQueryEntity::find()
            .filter(SlowQueryColumn::FingerprintId.eq(Some(fingerprint_id)))
            .order_by_desc(SlowQueryColumn::EventTimestamp)
            .limit(limit)
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to find slow query events by fingerprint: {}", e))
    }
}