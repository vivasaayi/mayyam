use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "aws_cost_insights")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub anomaly_id: Option<Uuid>,
    pub aggregate_id: Option<Uuid>,
    pub account_id: String,
    pub insight_type: String,
    pub prompt_template: String,
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_response: String,
    pub summary: Option<String>,
    pub recommendations: Option<Json>,
    pub confidence_score: Option<Decimal>,
    pub tokens_used: Option<i32>,
    pub processing_time_ms: Option<i32>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Domain model for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostInsightModel {
    pub id: Uuid,
    pub anomaly_id: Option<Uuid>,
    pub aggregate_id: Option<Uuid>,
    pub account_id: String,
    pub insight_type: String,
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_response: String,
    pub summary: Option<String>,
    pub recommendations: Option<serde_json::Value>,
    pub confidence_score: Option<f64>,
    pub tokens_used: Option<i32>,
    pub processing_time_ms: Option<i32>,
    pub created_at: String, // ISO datetime format
}

impl From<Model> for CostInsightModel {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            anomaly_id: model.anomaly_id,
            aggregate_id: model.aggregate_id,
            account_id: model.account_id,
            insight_type: model.insight_type,
            llm_provider: model.llm_provider,
            llm_model: model.llm_model,
            llm_response: model.llm_response,
            summary: model.summary,
            recommendations: model.recommendations.map(|j| j.into()),
            confidence_score: model
                .confidence_score
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            tokens_used: model.tokens_used,
            processing_time_ms: model.processing_time_ms,
            created_at: model.created_at.to_rfc3339(),
        }
    }
}
