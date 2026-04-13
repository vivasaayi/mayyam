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
#[sea_orm(table_name = "chaos_metrics_aggregates")]
pub struct Model {
    #[sea_orm(primary_key)]
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

    #[sea_orm(column_type = "TimestampWithTimeZone", nullable)]
    pub aggregation_start_at: Option<DateTime<Utc>>,
    #[sea_orm(column_type = "TimestampWithTimeZone", nullable)]
    pub aggregation_end_at: Option<DateTime<Utc>>,

    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub updated_at: DateTime<Utc>,
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

impl From<Model> for AggregateModel {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            aggregation_type: model.aggregation_type,
            experiment_id: model.experiment_id,
            resource_type: model.resource_type,
            total_runs: model.total_runs,
            successful_runs: model.successful_runs,
            failed_runs: model.failed_runs,
            success_rate_percent: model.success_rate_percent,
            avg_execution_duration_ms: model.avg_execution_duration_ms,
            max_execution_duration_ms: model.max_execution_duration_ms,
            min_execution_duration_ms: model.min_execution_duration_ms,
            avg_recovery_time_ms: model.avg_recovery_time_ms,
            max_recovery_time_ms: model.max_recovery_time_ms,
            avg_impact_severity: model.avg_impact_severity,
            most_common_failure_reason: model.most_common_failure_reason,
            rollback_success_rate_percent: model.rollback_success_rate_percent,
            aggregation_start_at: model.aggregation_start_at,
            aggregation_end_at: model.aggregation_end_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}