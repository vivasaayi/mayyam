use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "slow_query_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub event_timestamp: NaiveDateTime,
    pub query_time: f64, // seconds
    pub lock_time: Option<f64>, // seconds
    pub rows_sent: Option<i64>,
    pub rows_examined: Option<i64>,
    pub user_host: Option<String>,
    pub database: Option<String>,
    pub sql_text: String,
    pub raw_log_line: String,
    pub fingerprint_id: Option<Uuid>,
    pub parsed_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::aurora_cluster::Entity",
        from = "Column::ClusterId",
        to = "super::aurora_cluster::Column::Id"
    )]
    AuroraCluster,
    #[sea_orm(
        belongs_to = "super::query_fingerprint::Entity",
        from = "Column::FingerprintId",
        to = "super::query_fingerprint::Column::Id"
    )]
    QueryFingerprint,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryEventDto {
    pub cluster_id: Uuid,
    pub event_timestamp: NaiveDateTime,
    pub query_time: f64,
    pub lock_time: Option<f64>,
    pub rows_sent: Option<i64>,
    pub rows_examined: Option<i64>,
    pub user_host: Option<String>,
    pub database: Option<String>,
    pub sql_text: String,
    pub raw_log_line: String,
}

impl SlowQueryEventDto {
    pub fn into_active_model(self) -> ActiveModel {
        ActiveModel {
            id: Set(Uuid::new_v4()),
            cluster_id: Set(self.cluster_id),
            event_timestamp: Set(self.event_timestamp),
            query_time: Set(self.query_time),
            lock_time: Set(self.lock_time),
            rows_sent: Set(self.rows_sent),
            rows_examined: Set(self.rows_examined),
            user_host: Set(self.user_host),
            database: Set(self.database),
            sql_text: Set(self.sql_text),
            raw_log_line: Set(self.raw_log_line),
            fingerprint_id: Set(None),
            parsed_at: Set(Utc::now().naive_utc()),
            created_at: Set(Utc::now().naive_utc()),
        }
    }
}

pub type SlowQueryEvent = Model;