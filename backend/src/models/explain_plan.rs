use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "explain_plans")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub fingerprint_id: Uuid,
    pub cluster_id: Uuid,
    pub sql_text: String,
    pub plan_format: String, // json, text
    pub plan_data: String, // The actual EXPLAIN output
    pub engine_version: String,
    pub captured_at: NaiveDateTime,
    pub is_before_optimization: bool, // For before/after comparisons
    pub has_full_scan: bool,
    pub has_filesort: bool,
    pub has_temp_table: bool,
    pub estimated_rows: Option<i64>,
    pub actual_rows: Option<i64>,
    pub execution_time: Option<f64>,
    pub created_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::query_fingerprint::Entity",
        from = "Column::FingerprintId",
        to = "super::query_fingerprint::Column::Id"
    )]
    QueryFingerprint,
    #[sea_orm(
        belongs_to = "super::aurora_cluster::Entity",
        from = "Column::ClusterId",
        to = "super::aurora_cluster::Column::Id"
    )]
    AuroraCluster,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::query_fingerprint::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::QueryFingerprint.def()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainPlanDto {
    pub fingerprint_id: Uuid,
    pub cluster_id: Uuid,
    pub sql_text: String,
    pub plan_format: String,
    pub plan_data: String,
    pub engine_version: String,
    pub is_before_optimization: bool,
}

impl ExplainPlanDto {
    pub fn into_active_model(self) -> ActiveModel {
        ActiveModel {
            id: Set(Uuid::new_v4()),
            fingerprint_id: Set(self.fingerprint_id),
            cluster_id: Set(self.cluster_id),
            sql_text: Set(self.sql_text),
            plan_format: Set(self.plan_format),
            plan_data: Set(self.plan_data),
            engine_version: Set(self.engine_version),
            captured_at: Set(Utc::now().naive_utc()),
            is_before_optimization: Set(self.is_before_optimization),
            has_full_scan: Set(false), // Will be analyzed from plan
            has_filesort: Set(false),
            has_temp_table: Set(false),
            estimated_rows: Set(None),
            actual_rows: Set(None),
            execution_time: Set(None),
            created_at: Set(Utc::now().naive_utc()),
        }
    }
}

pub type ExplainPlan = Model;