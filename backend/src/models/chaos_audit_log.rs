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
#[sea_orm(table_name = "chaos_audit_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub experiment_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
    pub action: String,
    pub user_id: Option<String>,
    pub triggered_by: Option<String>,
    pub resource_id: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub old_values: serde_json::Value,
    #[sea_orm(column_type = "JsonBinary")]
    pub new_values: serde_json::Value,
    pub status_before: Option<String>,
    pub status_after: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosAuditAction;

impl ChaosAuditAction {
    pub const TEMPLATE_CREATED: &'static str = "template_created";
    pub const TEMPLATE_UPDATED: &'static str = "template_updated";
    pub const TEMPLATE_DELETED: &'static str = "template_deleted";

    pub const EXPERIMENT_CREATED: &'static str = "experiment_created";
    pub const EXPERIMENT_UPDATED: &'static str = "experiment_updated";
    pub const EXPERIMENT_DELETED: &'static str = "experiment_deleted";

    pub const RUN_STARTED: &'static str = "run_started";
    pub const RUN_COMPLETED: &'static str = "run_completed";
    pub const RUN_FAILED: &'static str = "run_failed";
    pub const RUN_STOPPED: &'static str = "run_stopped";
    pub const RUN_TIMED_OUT: &'static str = "run_timed_out";

    pub const ROLLBACK_STARTED: &'static str = "rollback_started";
    pub const ROLLBACK_COMPLETED: &'static str = "rollback_completed";
    pub const ROLLBACK_FAILED: &'static str = "rollback_failed";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogCreateDto {
    pub experiment_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
    pub action: String,
    pub user_id: Option<String>,
    pub triggered_by: Option<String>,
    pub resource_id: Option<String>,
    pub old_values: Option<serde_json::Value>,
    pub new_values: Option<serde_json::Value>,
    pub status_before: Option<String>,
    pub status_after: Option<String>,
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogQuery {
    pub experiment_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
    pub action: Option<String>,
    pub user_id: Option<String>,
    pub resource_id: Option<String>,
    pub triggered_by: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogPage {
    pub logs: Vec<Model>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}
