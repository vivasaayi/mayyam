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
#[sea_orm(table_name = "chaos_experiment_results")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub run_id: Uuid,
    pub experiment_id: Uuid,
    pub resource_id: String,
    pub resource_type: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub baseline_metrics: serde_json::Value,
    #[sea_orm(column_type = "JsonBinary")]
    pub during_metrics: serde_json::Value,
    #[sea_orm(column_type = "JsonBinary")]
    pub recovery_metrics: serde_json::Value,
    pub impact_summary: Option<String>,
    pub impact_severity: String,
    pub recovery_time_ms: Option<i64>,
    pub steady_state_hypothesis: Option<String>,
    pub hypothesis_met: Option<bool>,
    #[sea_orm(column_type = "JsonBinary")]
    pub observations: serde_json::Value,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::chaos_experiment_run::Entity",
        from = "Column::RunId",
        to = "super::chaos_experiment_run::Column::Id"
    )]
    Run,
    #[sea_orm(
        belongs_to = "super::chaos_experiment::Entity",
        from = "Column::ExperimentId",
        to = "super::chaos_experiment::Column::Id"
    )]
    Experiment,
}

impl Related<super::chaos_experiment_run::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Run.def()
    }
}

impl Related<super::chaos_experiment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Experiment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceExperimentHistory {
    pub resource_id: String,
    pub resource_type: String,
    pub experiments: Vec<ResourceExperimentSummary>,
    pub total_runs: u64,
    pub last_run_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceExperimentSummary {
    pub experiment_id: Uuid,
    pub experiment_name: String,
    pub experiment_type: String,
    pub run_id: Uuid,
    pub run_status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub impact_severity: String,
    pub recovery_time_ms: Option<i64>,
}
