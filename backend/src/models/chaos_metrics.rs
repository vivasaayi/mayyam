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

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "chaos_execution_metrics")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub run_id: Uuid,
    pub experiment_id: Uuid,
    pub resource_id: String,
    pub resource_type: String,

    pub execution_duration_ms: Option<i64>,
    pub rollback_duration_ms: Option<i64>,
    pub total_duration_ms: Option<i64>,

    pub execution_success: Option<bool>,
    pub rollback_success: Option<bool>,

    pub impact_severity: Option<String>,
    pub estimated_affected_resources: Option<i32>,
    pub confirmed_affected_resources: Option<i32>,

    pub time_to_first_error_ms: Option<i64>,
    pub time_to_recovery_ms: Option<i64>,
    pub recovery_completeness_percent: Option<i32>,

    pub api_calls_made: Option<i32>,
    pub api_errors: Option<i32>,
    pub retries_performed: Option<i32>,

    #[sea_orm(column_type = "JsonBinary")]
    pub custom_metrics: serde_json::Value,

    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateModel {
    pub id: Uuid,
    pub aggregation_type: String,
    pub experiment_id: Option<Uuid>,
    pub resource_type: Option<String>,

    pub total_runs: i64,
    pub successful_runs: i64,
    pub failed_runs: i64,
    pub success_rate_percent: Option<i32>,

    pub avg_execution_duration_ms: Option<i64>,
    pub max_execution_duration_ms: Option<i64>,
    pub min_execution_duration_ms: Option<i64>,

    pub avg_recovery_time_ms: Option<i64>,
    pub max_recovery_time_ms: Option<i64>,

    pub avg_impact_severity: Option<String>,
    pub most_common_failure_reason: Option<String>,

    pub rollback_success_rate_percent: Option<i32>,

    pub aggregation_start_at: Option<DateTime<Utc>>,
    pub aggregation_end_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// DTOs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetricsCreateDto {
    pub run_id: Uuid,
    pub experiment_id: Uuid,
    pub resource_id: String,
    pub resource_type: String,
    pub execution_duration_ms: Option<i64>,
    pub rollback_duration_ms: Option<i64>,
    pub execution_success: Option<bool>,
    pub rollback_success: Option<bool>,
    pub impact_severity: Option<String>,
    pub time_to_recovery_ms: Option<i64>,
    pub api_calls_made: Option<i32>,
    pub api_errors: Option<i32>,
    pub custom_metrics: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQuery {
    pub experiment_id: Option<Uuid>,
    pub resource_type: Option<String>,
    pub impact_severity: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsStats {
    pub total_experiments: u64,
    pub successful_experiments: u64,
    pub failed_experiments: u64,
    pub success_rate_percent: f64,

    pub avg_execution_duration_ms: f64,
    pub avg_recovery_time_ms: f64,

    pub most_impacted_resource_type: Option<String>,
    pub avg_impact_severity: Option<String>,

    pub rollback_success_rate_percent: f64,
    pub avg_rollback_time_ms: f64,
}
