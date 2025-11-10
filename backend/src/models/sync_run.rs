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
#[sea_orm(table_name = "sync_runs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid, // sync_id
    pub name: String,
    pub aws_account_id: Option<Uuid>,
    pub account_id: Option<String>,
    pub profile: Option<String>,
    pub region: Option<String>,
    pub status: String, // created|running|completed|failed
    pub total_resources: i32,
    pub success_count: i32,
    pub failure_count: i32,
    pub error_summary: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub metadata: serde_json::Value,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRunCreateDto {
    pub name: String,
    pub aws_account_id: Option<Uuid>,
    pub account_id: Option<String>,
    pub profile: Option<String>,
    pub region: Option<String>,
    // Optional region controls; stored into metadata by repository
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regions: Option<Vec<String>>, // explicit regions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_regions: Option<bool>, // if true, scan all regions
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRunDto {
    pub id: Uuid,
    pub name: String,
    pub aws_account_id: Option<Uuid>,
    pub account_id: Option<String>,
    pub profile: Option<String>,
    pub region: Option<String>,
    pub status: String,
    pub total_resources: i32,
    pub success_count: i32,
    pub failure_count: i32,
    pub error_summary: Option<String>,
    pub metadata: serde_json::Value,
    // Convenience fields parsed from metadata for UI
    pub region_scope: Option<String>, // "all" | "custom" | "enabled" (if we later encode)
    pub regions: Option<Vec<String>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Model> for SyncRunDto {
    fn from(m: Model) -> Self {
        // Derive convenience fields
        let (region_scope, regions) = {
            let mut scope: Option<String> = None;
            let mut regions_vec: Option<Vec<String>> = None;
            if let Some(true) = m.metadata.get("all_regions").and_then(|v| v.as_bool()) {
                scope = Some("all".to_string());
            }
            if let Some(arr) = m.metadata.get("regions").and_then(|v| v.as_array()) {
                let r = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>();
                if !r.is_empty() {
                    regions_vec = Some(r);
                    if scope.is_none() {
                        scope = Some("custom".to_string());
                    }
                }
            }
            (scope, regions_vec)
        };

        Self {
            id: m.id,
            name: m.name,
            aws_account_id: m.aws_account_id,
            account_id: m.account_id,
            profile: m.profile,
            region: m.region,
            status: m.status,
            total_resources: m.total_resources,
            success_count: m.success_count,
            failure_count: m.failure_count,
            error_summary: m.error_summary,
            metadata: m.metadata,
            region_scope,
            regions,
            started_at: m.started_at,
            completed_at: m.completed_at,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncRunQueryParams {
    pub status: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}
