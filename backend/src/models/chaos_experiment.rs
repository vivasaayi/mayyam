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
#[sea_orm(table_name = "chaos_experiments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub template_id: Option<Uuid>,
    pub account_id: String,
    pub region: String,
    pub resource_type: String,
    pub target_resource_id: String,
    pub target_resource_name: Option<String>,
    pub experiment_type: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub parameters: serde_json::Value,
    pub schedule_cron: Option<String>,
    pub status: String,
    pub created_by: Option<String>,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::chaos_template::Entity",
        from = "Column::TemplateId",
        to = "super::chaos_template::Column::Id"
    )]
    Template,
    #[sea_orm(has_many = "super::chaos_experiment_run::Entity")]
    Runs,
    #[sea_orm(has_many = "super::chaos_experiment_result::Entity")]
    Results,
}

impl Related<super::chaos_template::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Template.def()
    }
}

impl Related<super::chaos_experiment_run::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Runs.def()
    }
}

impl Related<super::chaos_experiment_result::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Results.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentStatus;

impl ExperimentStatus {
    pub const DRAFT: &'static str = "draft";
    pub const READY: &'static str = "ready";
    pub const SCHEDULED: &'static str = "scheduled";
    pub const RUNNING: &'static str = "running";
    pub const COMPLETED: &'static str = "completed";
    pub const FAILED: &'static str = "failed";
    pub const CANCELLED: &'static str = "cancelled";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosExperimentCreateDto {
    pub name: String,
    pub description: Option<String>,
    pub template_id: Option<Uuid>,
    pub account_id: String,
    pub region: String,
    pub resource_type: String,
    pub target_resource_id: String,
    pub target_resource_name: Option<String>,
    pub experiment_type: String,
    pub parameters: Option<serde_json::Value>,
    pub schedule_cron: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosExperimentUpdateDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub schedule_cron: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosExperimentQuery {
    pub account_id: Option<String>,
    pub region: Option<String>,
    pub resource_type: Option<String>,
    pub target_resource_id: Option<String>,
    pub experiment_type: Option<String>,
    pub status: Option<String>,
    pub template_id: Option<Uuid>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosExperimentPage {
    pub experiments: Vec<Model>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosExperimentWithRuns {
    #[serde(flatten)]
    pub experiment: Model,
    pub last_run: Option<super::chaos_experiment_run::Model>,
    pub total_runs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRunRequest {
    pub experiment_ids: Vec<Uuid>,
    pub triggered_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunExperimentRequest {
    pub triggered_by: Option<String>,
    pub parameter_overrides: Option<serde_json::Value>,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}
