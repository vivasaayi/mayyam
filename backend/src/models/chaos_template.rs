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
#[sea_orm(table_name = "chaos_experiment_templates")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub resource_type: String,
    pub experiment_type: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub default_parameters: serde_json::Value,
    pub prerequisites: Option<Vec<String>>,
    pub expected_impact: String,
    pub estimated_duration_seconds: i32,
    #[sea_orm(column_type = "JsonBinary")]
    pub rollback_steps: serde_json::Value,
    pub documentation: Option<String>,
    pub is_built_in: bool,
    pub is_active: bool,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::chaos_experiment::Entity")]
    ChaosExperiments,
}

impl Related<super::chaos_experiment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChaosExperiments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosTemplateCreateDto {
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub resource_type: String,
    pub experiment_type: String,
    pub default_parameters: Option<serde_json::Value>,
    pub prerequisites: Option<Vec<String>>,
    pub expected_impact: Option<String>,
    pub estimated_duration_seconds: Option<i32>,
    pub rollback_steps: Option<serde_json::Value>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosTemplateUpdateDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub default_parameters: Option<serde_json::Value>,
    pub prerequisites: Option<Vec<String>>,
    pub expected_impact: Option<String>,
    pub estimated_duration_seconds: Option<i32>,
    pub rollback_steps: Option<serde_json::Value>,
    pub documentation: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosTemplateQuery {
    pub category: Option<String>,
    pub resource_type: Option<String>,
    pub experiment_type: Option<String>,
    pub is_built_in: Option<bool>,
    pub is_active: Option<bool>,
}
