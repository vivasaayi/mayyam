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
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Model> for SyncRunDto {
    fn from(m: Model) -> Self {
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
