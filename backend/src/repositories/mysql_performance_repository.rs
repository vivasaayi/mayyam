use crate::models::mysql_performance_snapshot::{MySQLPerformanceSnapshot, Entity as MySQLPerformanceEntity, Column as MySQLPerformanceColumn};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, PaginatorTrait};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{NaiveDateTime, Duration};

#[derive(Clone)]
pub struct MySQLPerformanceRepository {
    db: Arc<DatabaseConnection>,
}

impl MySQLPerformanceRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, snapshot: MySQLPerformanceSnapshot) -> Result<MySQLPerformanceSnapshot, String> {
        let active_model: crate::models::mysql_performance_snapshot::ActiveModel = snapshot.into();
        active_model.insert(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to create performance snapshot: {}", e))
    }

    pub async fn find_by_cluster(&self, cluster_id: Uuid) -> Result<Vec<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceSnapshotEntity::find()
            .filter(MySQLPerformanceSnapshotColumn::ClusterId.eq(cluster_id))
            .order_by_desc(MySQLPerformanceSnapshotColumn::CapturedAt)
            .all(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find performance snapshots: {}", e))
    }

    pub async fn find_by_id(&self, snapshot_id: Uuid) -> Result<Option<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceSnapshotEntity::find_by_id(snapshot_id)
            .one(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find performance snapshot: {}", e))
    }

    pub async fn find_latest_by_cluster(&self, cluster_id: Uuid) -> Result<Option<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceSnapshotEntity::find()
            .filter(MySQLPerformanceSnapshotColumn::ClusterId.eq(cluster_id))
            .order_by_desc(MySQLPerformanceSnapshotColumn::CapturedAt)
            .one(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find latest snapshot: {}", e))
    }

    pub async fn find_by_time_range(
        &self,
        cluster_id: Uuid,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Vec<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceSnapshotEntity::find()
            .filter(MySQLPerformanceSnapshotColumn::ClusterId.eq(cluster_id))
            .filter(MySQLPerformanceSnapshotColumn::CapturedAt.gte(start_time))
            .filter(MySQLPerformanceSnapshotColumn::CapturedAt.lte(end_time))
            .order_by_desc(MySQLPerformanceSnapshotColumn::CapturedAt)
            .all(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find snapshots by time range: {}", e))
    }

    pub async fn find_with_health_score_below(
        &self,
        cluster_id: Uuid,
        threshold: f64,
        hours: i64,
    ) -> Result<Vec<MySQLPerformanceSnapshot>, String> {
        let cutoff_time = chrono::Utc::now().naive_utc() - Duration::hours(hours);

        MySQLPerformanceSnapshotEntity::find()
            .filter(MySQLPerformanceSnapshotColumn::ClusterId.eq(cluster_id))
            .filter(MySQLPerformanceSnapshotColumn::CapturedAt.gte(cutoff_time))
            .filter(MySQLPerformanceSnapshotColumn::OverallHealthScore.lt(threshold))
            .order_by_desc(MySQLPerformanceSnapshotColumn::CapturedAt)
            .all(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find unhealthy snapshots: {}", e))
    }

    pub async fn update_health_score(&self, snapshot_id: Uuid, score: f64) -> Result<(), String> {
        let mut active_model = MySQLPerformanceSnapshotEntity::find_by_id(snapshot_id)
            .one(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find snapshot: {}", e))?
            .ok_or_else(|| "Performance snapshot not found".to_string())?
            .into_active_model();

        active_model.overall_health_score = Set(score);
        active_model.update(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to update health score: {}", e))?;
        Ok(())
    }

    pub async fn count_by_cluster(&self, cluster_id: Uuid) -> Result<u64, String> {
        MySQLPerformanceSnapshotEntity::find()
            .filter(MySQLPerformanceSnapshotColumn::ClusterId.eq(cluster_id))
            .count(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to count snapshots: {}", e))
    }

    pub async fn delete_old_snapshots(&self, days_to_keep: i64) -> Result<u64, String> {
        let cutoff_date = chrono::Utc::now().naive_utc() - Duration::days(days_to_keep);

        let delete_result = MySQLPerformanceSnapshotEntity::delete_many()
            .filter(MySQLPerformanceSnapshotColumn::CapturedAt.lt(cutoff_date))
            .exec(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to delete old snapshots: {}", e))?;

        Ok(delete_result.rows_affected)
    }
}