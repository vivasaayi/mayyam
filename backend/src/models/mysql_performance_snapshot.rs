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
#[sea_orm(table_name = "mysql_performance_snapshots")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub snapshot_time: NaiveDateTime,

    // Workload metrics
    pub qps: f64,
    pub tps: f64,
    pub threads_running: i32,
    pub threads_connected: i32,
    pub connections_used: f64, // percentage

    // Slow query metrics
    pub slow_queries_total: i64,
    pub slow_query_time_total: f64,
    pub slow_query_p95: f64,

    // InnoDB metrics
    pub innodb_buffer_pool_usage: f64, // percentage
    pub innodb_log_file_usage: f64,
    pub innodb_history_length: i64,
    pub innodb_flushes: i64,

    // Temporary tables
    pub temp_tables_disk: i64,
    pub temp_tables_memory: i64,

    // Replication
    pub replication_lag: Option<f64>, // seconds

    // Health score
    pub health_score: String, // A, B, C, D, F
    pub top_issues: Json, // Array of top issues

    pub created_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::aurora_cluster::Entity",
        from = "Column::ClusterId",
        to = "super::aurora_cluster::Column::Id"
    )]
    AuroraCluster,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySQLPerformanceSnapshotDto {
    pub cluster_id: Uuid,
    pub qps: f64,
    pub tps: f64,
    pub threads_running: i32,
    pub threads_connected: i32,
    pub connections_used: f64,
    pub slow_queries_total: i64,
    pub slow_query_time_total: f64,
    pub slow_query_p95: f64,
    pub innodb_buffer_pool_usage: f64,
    pub innodb_log_file_usage: f64,
    pub innodb_history_length: i64,
    pub innodb_flushes: i64,
    pub temp_tables_disk: i64,
    pub temp_tables_memory: i64,
    pub replication_lag: Option<f64>,
    pub health_score: String,
    pub top_issues: Vec<String>,
}

impl MySQLPerformanceSnapshotDto {
    pub fn into_active_model(self) -> ActiveModel {
        ActiveModel {
            id: Set(Uuid::new_v4()),
            cluster_id: Set(self.cluster_id),
            snapshot_time: Set(Utc::now().naive_utc()),
            qps: Set(self.qps),
            tps: Set(self.tps),
            threads_running: Set(self.threads_running),
            threads_connected: Set(self.threads_connected),
            connections_used: Set(self.connections_used),
            slow_queries_total: Set(self.slow_queries_total),
            slow_query_time_total: Set(self.slow_query_time_total),
            slow_query_p95: Set(self.slow_query_p95),
            innodb_buffer_pool_usage: Set(self.innodb_buffer_pool_usage),
            innodb_log_file_usage: Set(self.innodb_log_file_usage),
            innodb_history_length: Set(self.innodb_history_length),
            innodb_flushes: Set(self.innodb_flushes),
            temp_tables_disk: Set(self.temp_tables_disk),
            temp_tables_memory: Set(self.temp_tables_memory),
            replication_lag: Set(self.replication_lag),
            health_score: Set(self.health_score),
            top_issues: Set(serde_json::to_value(&self.top_issues).unwrap_or(serde_json::Value::Array(vec![]))),
            created_at: Set(Utc::now().naive_utc()),
        }
    }
}

pub type MySQLPerformanceSnapshot = Model;