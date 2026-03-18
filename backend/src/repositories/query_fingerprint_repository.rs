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
        rows_examined: i64,
        rows_sent: i64,
        last_seen: NaiveDateTime,
    ) -> Result<(), String> {
        let mut active_model = QueryFingerprintEntity::find_by_id(fingerprint_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to find query fingerprint: {}", e))?
            .ok_or_else(|| "Query fingerprint not found".to_string())?
            .into_active_model();

        let waste_score = (rows_examined as f64 + 1.0) / (rows_sent as f64 + 1.0);

        active_model.execution_count = Set(execution_count);
        active_model.total_query_time = Set(total_time);
        active_model.avg_query_time = Set(avg_time);
        active_model.total_rows_examined = Set(rows_examined);
        active_model.total_rows_sent = Set(rows_sent);
        active_model.waste_score = Set(waste_score);
        active_model.last_seen = Set(last_seen);

        active_model.update(self.db.as_ref())
            .await
            .map_err(|e| format!("Failed to update statistics: {}", e))?;
        Ok(())
    }

    pub async fn update_stats_batch(
        &self,
        updates: Vec<(Uuid, i64, f64, f64, i64, i64, NaiveDateTime)>,
    ) -> Result<(), String> {
        for (fingerprint_id, execution_count, total_time, avg_time, rows_ex, rows_sent, last_seen) in updates {
            self.update_stats(fingerprint_id, execution_count, total_time, avg_time, rows_ex, rows_sent, last_seen).await?;
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

    pub async fn get_top_offending_tables(
        &self,
        _cluster_id: Option<Uuid>,
        hours: i64,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>, String> {
        let fingerprints = self.find_top_by_execution_time(_cluster_id, hours, 1000).await?;
        
        let mut table_stats: std::collections::HashMap<String, (f64, i64, f64)> = std::collections::HashMap::new();

        for fp in fingerprints {
            if let Some(tables) = fp.tables_used.as_array() {
                for table_val in tables {
                    if let Some(table_name) = table_val.as_str() {
                        let entry = table_stats.entry(table_name.to_string()).or_insert((0.0, 0, 0.0));
                        entry.0 += fp.total_query_time;
                        entry.1 += fp.execution_count;
                        entry.2 += fp.waste_score * fp.execution_count as f64;
                    }
                }
            }
        }

        let mut result: Vec<serde_json::Value> = table_stats.into_iter()
            .map(|(table_name, (total_time, exec_count, total_waste))| {
                serde_json::json!({
                    "table_name": table_name,
                    "total_query_time": total_time,
                    "execution_count": exec_count,
                    "avg_waste_score": if exec_count > 0 { total_waste / exec_count as f64 } else { 0.0 }
                })
            })
            .collect();

        result.sort_by(|a, b| {
            let a_time = a["total_query_time"].as_f64().unwrap_or(0.0);
            let b_time = b["total_query_time"].as_f64().unwrap_or(0.0);
            b_time.partial_cmp(&a_time).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(result.into_iter().take(limit).collect())
    }
}