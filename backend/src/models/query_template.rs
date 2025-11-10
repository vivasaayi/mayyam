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

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Deserialize, Serialize)]
#[sea_orm(table_name = "query_templates")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub query: String,
    pub description: Option<String>,
    pub connection_type: Option<String>, // e.g., "mysql", "postgresql", or NULL for common templates
    pub category: Option<String>,        // e.g., "Performance", "Schema", "Monitoring"
    pub is_favorite: bool,
    pub display_order: i32,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// DTOs for query template operations
#[derive(Debug, Deserialize)]
pub struct CreateQueryTemplateRequest {
    pub name: String,
    pub query: String,
    pub description: Option<String>,
    pub connection_type: Option<String>, // Make connection_type optional
    pub category: Option<String>,
    pub is_favorite: Option<bool>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateQueryTemplateRequest {
    pub name: Option<String>,
    pub query: Option<String>,
    pub description: Option<String>,
    pub connection_type: Option<String>, // Keep it optional
    pub category: Option<String>,
    pub is_favorite: Option<bool>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct QueryTemplateResponse {
    pub id: Uuid,
    pub name: String,
    pub query: String,
    pub description: Option<String>,
    pub connection_type: Option<String>,
    pub category: Option<String>,
    pub is_favorite: bool,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
