use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_analyses")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub fingerprint_id: Option<Uuid>,
    pub slow_query_id: Option<Uuid>,
    pub ai_provider: String, // openai, local_llm, none
    pub ai_model: String,
    pub analysis_type: String, // summary, recommendations, jira_ticket
    pub input_data: Json, // Sanitized input sent to AI
    pub analysis_result: String, // AI-generated analysis
    pub confidence_score: Option<f64>, // 0.0 to 1.0
    pub suggested_indexes: Json, // Array of suggested index definitions
    pub suggested_rewrites: Json, // Array of suggested SQL rewrites
    pub root_causes: Json, // Array of identified root causes
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
        belongs_to = "super::slow_query_event::Entity",
        from = "Column::SlowQueryId",
        to = "super::slow_query_event::Column::Id"
    )]
    SlowQueryEvent,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIAnalysisDto {
    pub cluster_id: Uuid,
    pub fingerprint_id: Option<Uuid>,
    pub slow_query_id: Option<Uuid>,
    pub ai_provider: String,
    pub ai_model: String,
    pub analysis_type: String,
    pub input_data: serde_json::Value,
    pub analysis_result: String,
    pub confidence_score: Option<f64>,
    pub suggested_indexes: Vec<String>,
    pub suggested_rewrites: Vec<String>,
    pub root_causes: Vec<String>,
}

impl AIAnalysisDto {
    pub fn into_active_model(self) -> ActiveModel {
        ActiveModel {
            id: Set(Uuid::new_v4()),
            cluster_id: Set(self.cluster_id),
            fingerprint_id: Set(self.fingerprint_id),
            slow_query_id: Set(self.slow_query_id),
            ai_provider: Set(self.ai_provider),
            ai_model: Set(self.ai_model),
            analysis_type: Set(self.analysis_type),
            input_data: Set(self.input_data),
            analysis_result: Set(self.analysis_result),
            confidence_score: Set(self.confidence_score),
            suggested_indexes: Set(serde_json::to_value(&self.suggested_indexes).unwrap_or(serde_json::Value::Array(vec![]))),
            suggested_rewrites: Set(serde_json::to_value(&self.suggested_rewrites).unwrap_or(serde_json::Value::Array(vec![]))),
            root_causes: Set(serde_json::to_value(&self.root_causes).unwrap_or(serde_json::Value::Array(vec![]))),
            created_at: Set(Utc::now().naive_utc()),
        }
    }
}

pub type AIAnalysis = Model;