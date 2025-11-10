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


use crate::models::query_fingerprint::{QueryFingerprint, Entity as QueryFingerprintEntity, Column as QueryFingerprintColumn};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, PaginatorTrait, QueryOrder, QuerySelect, IntoActiveModel};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{NaiveDateTime, Duration};

#[derive(Clone)]
pub struct QueryFingerprintRepository {
    db: Arc<DatabaseConnection>,
}

impl QueryFingerprintRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, fingerprint: QueryFingerprint) -> Result<QueryFingerprint, String> {
        let active_model: crate::models::query_fingerprint::ActiveModel = fingerprint.into();
        active_model.insert(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to create query fingerprint: {}", e))
    }

    pub async fn find_by_hash(&self, _cluster_id: Uuid, query_hash: &str) -> Result<Option<QueryFingerprint>, String> {
        QueryFingerprintEntity::find()
            .filter(QueryFingerprintColumn::FingerprintHash.eq(query_hash))
            .one(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find query fingerprint: {}", e))
    }

    pub async fn find_by_id(&self, fingerprint_id: Uuid) -> Result<Option<QueryFingerprint>, String> {
        QueryFingerprintEntity::find_by_id(fingerprint_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find query fingerprint: {}", e))
    }

    pub async fn find_by_cluster(&self, _cluster_id: Uuid) -> Result<Vec<QueryFingerprint>, String> {
        QueryFingerprintEntity::find()
            .order_by_desc(QueryFingerprintColumn::TotalQueryTime)
            .all(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find query fingerprints: {}", e))
    }

    pub async fn find_top_by_execution_time(
        &self,
        _cluster_id: Option<Uuid>,
        hours: i64,
        limit: u64,
    ) -> Result<Vec<QueryFingerprint>, String> {
        let cutoff_time = chrono::Utc::now().naive_utc() - Duration::hours(hours);

        QueryFingerprintEntity::find()
            .filter(QueryFingerprintColumn::LastSeen.gte(cutoff_time))
            .order_by_desc(QueryFingerprintColumn::TotalQueryTime)
            .limit(limit)
            .all(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find top fingerprints: {}", e))
    }

    pub async fn update_stats(
        &self,
        fingerprint_id: Uuid,
        execution_count: i64,
        total_time: f64,
        avg_time: f64,
        last_seen: NaiveDateTime,
    ) -> Result<(), String> {
        let mut active_model = QueryFingerprintEntity::find_by_id(fingerprint_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find query fingerprint: {}", e))?
            .ok_or_else(|| "Query fingerprint not found".to_string())?
            .into_active_model();

        active_model.execution_count = Set(execution_count);
        active_model.total_query_time = Set(total_time);
        active_model.avg_query_time = Set(avg_time);
        active_model.last_seen = Set(last_seen);

        active_model.update(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to update statistics: {}", e))?;
        Ok(())
    }

    pub async fn update_stats_batch(
        &self,
        updates: Vec<(Uuid, i64, f64, f64, NaiveDateTime)>,
    ) -> Result<(), String> {
        // For batch updates, we'll update each fingerprint individually
        // In a production system, you might want to use raw SQL for better performance
        for (fingerprint_id, execution_count, total_time, avg_time, last_seen) in updates {
            self.update_stats(fingerprint_id, execution_count, total_time, avg_time, last_seen).await?;
        }
        Ok(())
    }

    pub async fn update_catalog_data(
        &self,
        fingerprint_id: Uuid,
        tables: Vec<String>,
        columns: Vec<String>,
    ) -> Result<(), String> {
        let mut active_model = QueryFingerprintEntity::find_by_id(fingerprint_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find query fingerprint: {}", e))?
            .ok_or_else(|| "Query fingerprint not found".to_string())?
            .into_active_model();

        active_model.tables_used = Set(serde_json::to_value(tables)
            .map_err(|e| format!("Failed to serialize tables: {}", e))?);
        active_model.columns_used = Set(serde_json::to_value(columns)
            .map_err(|e| format!("Failed to serialize columns: {}", e))?);

        active_model.update(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to update catalog data: {}", e))?;
        Ok(())
    }

    pub async fn count_by_cluster(&self, _cluster_id: Uuid) -> Result<u64, String> {
        QueryFingerprintEntity::find()
            .count(&*self.db)
            .await
            .map_err(|e| format!("Failed to count fingerprints: {}", e))
    }

    pub async fn delete_unused(&self, days_since_last_seen: i64) -> Result<u64, String> {
        let cutoff_date = chrono::Utc::now().naive_utc() - Duration::days(days_since_last_seen);

        let delete_result = QueryFingerprintEntity::delete_many()
            .filter(QueryFingerprintColumn::LastSeen.lt(cutoff_date))
            .exec(&*self.db)
            .await
            .map_err(|e| format!("Failed to delete unused fingerprints: {}", e))?;

        Ok(delete_result.rows_affected)
    }

    pub async fn find_patterns_by_cluster(&self, _cluster_id: Uuid, hours: i64) -> Result<Vec<QueryFingerprint>, String> {
        let cutoff_time = chrono::Utc::now().naive_utc() - Duration::hours(hours);

        QueryFingerprintEntity::find()
            .filter(QueryFingerprintColumn::LastSeen.gte(cutoff_time))
            .order_by_desc(QueryFingerprintColumn::TotalQueryTime)
            .all(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find patterns by cluster: {}", e))
    }
}