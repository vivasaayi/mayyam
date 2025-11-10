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


use crate::models::mysql_performance_snapshot::{MySQLPerformanceSnapshot, Entity as MySQLPerformanceEntity, Column as MySQLPerformanceColumn};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, PaginatorTrait, QueryOrder, IntoActiveModel, QuerySelect};
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
        active_model.insert(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to create performance snapshot: {}", e))
    }

    pub async fn find_by_cluster(&self, cluster_id: Uuid) -> Result<Vec<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceEntity::find()
            .filter(MySQLPerformanceColumn::ClusterId.eq(cluster_id))
            .order_by_desc(MySQLPerformanceColumn::SnapshotTime)
            .all(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find performance snapshots: {}", e))
    }

    pub async fn find_by_id(&self, snapshot_id: Uuid) -> Result<Option<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceEntity::find_by_id(snapshot_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find performance snapshot: {}", e))
    }

    pub async fn find_latest_by_cluster(&self, cluster_id: Uuid) -> Result<Option<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceEntity::find()
            .filter(MySQLPerformanceColumn::ClusterId.eq(cluster_id))
            .order_by_desc(MySQLPerformanceColumn::SnapshotTime)
            .one(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find latest snapshot: {}", e))
    }

    pub async fn find_by_time_range(
        &self,
        cluster_id: Uuid,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Vec<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceEntity::find()
            .filter(MySQLPerformanceColumn::ClusterId.eq(cluster_id))
            .filter(MySQLPerformanceColumn::SnapshotTime.gte(start_time))
            .filter(MySQLPerformanceColumn::SnapshotTime.lte(end_time))
            .order_by_desc(MySQLPerformanceColumn::SnapshotTime)
            .all(self.db.as_ref())
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

        MySQLPerformanceEntity::find()
            .filter(MySQLPerformanceColumn::ClusterId.eq(cluster_id))
            .filter(MySQLPerformanceColumn::SnapshotTime.gte(cutoff_time))
            .filter(MySQLPerformanceColumn::HealthScore.lt(threshold))
            .order_by_desc(MySQLPerformanceColumn::SnapshotTime)
            .all(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find unhealthy snapshots: {}", e))
    }

    pub async fn update_health_score(&self, snapshot_id: Uuid, score: f64) -> Result<(), String> {
        let mut active_model = MySQLPerformanceEntity::find_by_id(snapshot_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find snapshot: {}", e))?
            .ok_or_else(|| "Performance snapshot not found".to_string())?
            .into_active_model();

        active_model.health_score = Set(score.to_string());
        active_model.update(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to update health score: {}", e))?;
        Ok(())
    }

    pub async fn count_by_cluster(&self, cluster_id: Uuid) -> Result<u64, String> {
        MySQLPerformanceEntity::find()
            .filter(MySQLPerformanceColumn::ClusterId.eq(cluster_id))
            .count(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to count snapshots: {}", e))
    }

    pub async fn delete_old_snapshots(&self, days_to_keep: i64) -> Result<u64, String> {
        let cutoff_date = chrono::Utc::now().naive_utc() - Duration::days(days_to_keep);

        let delete_result = MySQLPerformanceEntity::delete_many()
            .filter(MySQLPerformanceColumn::SnapshotTime.lt(cutoff_date))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to delete old snapshots: {}", e))?;

        Ok(delete_result.rows_affected)
    }

    pub async fn find_by_cluster_and_time(
        &self,
        cluster_id: Uuid,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Vec<MySQLPerformanceSnapshot>, String> {
        self.find_by_time_range(cluster_id, start_time, end_time).await
    }

    pub async fn find_recent(&self, limit: u64) -> Result<Vec<MySQLPerformanceSnapshot>, String> {
        MySQLPerformanceEntity::find()
            .order_by_desc(MySQLPerformanceColumn::SnapshotTime)
            .limit(limit)
            .all(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find recent snapshots: {}", e))
    }

    pub async fn get_average_health_score(&self) -> Result<f64, String> {
        // This would require aggregation query, for now return a placeholder
        Ok(0.85)
    }

    pub async fn delete(&self, snapshot_id: Uuid) -> Result<(), String> {
        MySQLPerformanceEntity::delete_by_id(snapshot_id)
            .exec(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to delete snapshot: {}", e))?;
        Ok(())
    }

    pub async fn cleanup_old_snapshots(&self, days_to_keep: i64) -> Result<u64, String> {
        self.delete_old_snapshots(days_to_keep).await
    }
}