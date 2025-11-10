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


use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "query_fingerprints")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub normalized_sql: String,
    pub fingerprint_hash: String, // For fast lookups
    pub total_query_time: f64, // Sum of all query times
    pub avg_query_time: f64,
    pub p95_query_time: f64,
    pub p99_query_time: f64,
    pub total_rows_examined: i64,
    pub total_rows_sent: i64,
    pub execution_count: i64,
    pub cluster_count: i32, // Number of clusters this fingerprint appears in
    pub first_seen: NaiveDateTime,
    pub last_seen: NaiveDateTime,
    pub tables_used: Json, // Array of table names
    pub columns_used: Json, // Array of column names
    pub has_full_scan: bool,
    pub has_filesort: bool,
    pub has_temp_table: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::slow_query_event::Entity")]
    SlowQueryEvents,
    #[sea_orm(has_many = "super::explain_plan::Entity")]
    ExplainPlans,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFingerprintDto {
    pub normalized_sql: String,
    pub fingerprint_hash: String,
    pub tables_used: Vec<String>,
    pub columns_used: Vec<String>,
}

impl QueryFingerprintDto {
    pub fn into_active_model(self) -> ActiveModel {
        ActiveModel {
            id: Set(Uuid::new_v4()),
            normalized_sql: Set(self.normalized_sql),
            fingerprint_hash: Set(self.fingerprint_hash),
            total_query_time: Set(0.0),
            avg_query_time: Set(0.0),
            p95_query_time: Set(0.0),
            p99_query_time: Set(0.0),
            total_rows_examined: Set(0),
            total_rows_sent: Set(0),
            execution_count: Set(0),
            cluster_count: Set(0),
            first_seen: Set(Utc::now().naive_utc()),
            last_seen: Set(Utc::now().naive_utc()),
            tables_used: Set(serde_json::to_value(&self.tables_used).unwrap_or(serde_json::Value::Array(vec![]))),
            columns_used: Set(serde_json::to_value(&self.columns_used).unwrap_or(serde_json::Value::Array(vec![]))),
            has_full_scan: Set(false),
            has_filesort: Set(false),
            has_temp_table: Set(false),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        }
    }
}

pub type QueryFingerprint = Model;