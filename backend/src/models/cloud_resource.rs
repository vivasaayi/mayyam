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
#[sea_orm(table_name = "cloud_resources")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub sync_id: Uuid,
    pub provider: String,
    pub account_id: String,
    pub region: String,
    pub resource_type: String,
    pub resource_id: String,
    pub arn_or_uri: Option<String>,
    pub name: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub tags: serde_json::Value,
    #[sea_orm(column_type = "JsonBinary")]
    pub resource_data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_refreshed: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudResourceDto {
    pub id: Option<Uuid>,
    pub sync_id: Uuid,
    pub provider: String,
    pub account_id: String,
    pub region: String,
    pub resource_type: String,
    pub resource_id: String,
    pub arn_or_uri: Option<String>,
    pub name: Option<String>,
    pub tags: serde_json::Value,
    pub resource_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudResourceQuery {
    pub sync_id: Option<Uuid>,
    pub provider: Option<String>,
    pub account_id: Option<String>,
    pub region: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub name: Option<String>,
    pub tag_key: Option<String>,
    pub tag_value: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudResourcePage {
    pub resources: Vec<Model>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

impl From<CloudResourceDto> for Model {
    fn from(dto: CloudResourceDto) -> Self {
        let now = Utc::now();
        Self {
            id: dto.id.unwrap_or_else(Uuid::new_v4),
            sync_id: dto.sync_id,
            provider: dto.provider,
            account_id: dto.account_id,
            region: dto.region,
            resource_type: dto.resource_type,
            resource_id: dto.resource_id,
            arn_or_uri: dto.arn_or_uri,
            name: dto.name,
            tags: dto.tags,
            resource_data: dto.resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now,
        }
    }
}

impl From<Model> for CloudResourceDto {
    fn from(model: Model) -> Self {
        Self {
            id: Some(model.id),
            sync_id: model.sync_id,
            provider: model.provider,
            account_id: model.account_id,
            region: model.region,
            resource_type: model.resource_type,
            resource_id: model.resource_id,
            arn_or_uri: model.arn_or_uri,
            name: model.name,
            tags: model.tags,
            resource_data: model.resource_data,
        }
    }
}
