use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "aurora_clusters")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub engine: String, // mysql, postgresql
    pub region: String,
    pub log_group: Option<String>,
    pub log_stream: Option<String>,
    pub read_only_dsn: String, // For EXPLAIN queries
    pub is_active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuroraClusterDto {
    pub name: String,
    pub engine: String,
    pub region: String,
    pub log_group: Option<String>,
    pub log_stream: Option<String>,
    pub read_only_dsn: String,
}

impl AuroraClusterDto {
    pub fn into_active_model(self) -> ActiveModel {
        ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(self.name),
            engine: Set(self.engine),
            region: Set(self.region),
            log_group: Set(self.log_group),
            log_stream: Set(self.log_stream),
            read_only_dsn: Set(self.read_only_dsn),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        }
    }
}

pub type AuroraCluster = Model;

impl From<Model> for AuroraClusterDto {
    fn from(model: Model) -> Self {
        AuroraClusterDto {
            name: model.name,
            engine: model.engine,
            region: model.region,
            log_group: model.log_group,
            log_stream: model.log_stream,
            read_only_dsn: model.read_only_dsn,
        }
    }
}