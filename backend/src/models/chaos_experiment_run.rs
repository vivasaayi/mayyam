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
#[sea_orm(table_name = "chaos_experiment_runs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub experiment_id: Uuid,
    pub run_number: i32,
    pub status: String,
    #[sea_orm(column_type = "TimestampWithTimeZone", nullable)]
    pub started_at: Option<DateTime<Utc>>,
    #[sea_orm(column_type = "TimestampWithTimeZone", nullable)]
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub triggered_by: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub execution_log: serde_json::Value,
    pub error_message: Option<String>,
    pub rollback_status: Option<String>,
    #[sea_orm(column_type = "TimestampWithTimeZone", nullable)]
    pub rollback_started_at: Option<DateTime<Utc>>,
    #[sea_orm(column_type = "TimestampWithTimeZone", nullable)]
    pub rollback_ended_at: Option<DateTime<Utc>>,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::chaos_experiment::Entity",
        from = "Column::ExperimentId",
        to = "super::chaos_experiment::Column::Id"
    )]
    Experiment,
    #[sea_orm(has_many = "super::chaos_experiment_result::Entity")]
    Results,
}

impl Related<super::chaos_experiment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Experiment.def()
    }
}

impl Related<super::chaos_experiment_result::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Results.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStatus;

impl RunStatus {
    pub const PENDING: &'static str = "pending";
    pub const INITIALIZING: &'static str = "initializing";
    pub const RUNNING: &'static str = "running";
    pub const ROLLING_BACK: &'static str = "rolling_back";
    pub const COMPLETED: &'static str = "completed";
    pub const FAILED: &'static str = "failed";
    pub const CANCELLED: &'static str = "cancelled";
    pub const TIMED_OUT: &'static str = "timed_out";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunWithResults {
    #[serde(flatten)]
    pub run: Model,
    pub results: Vec<super::chaos_experiment_result::Model>,
}
