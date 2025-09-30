use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "aws_monthly_cost_aggregates")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub account_id: String,
    pub service_name: String,
    pub month_year: Date,
    pub total_cost: Decimal,
    pub usage_amount: Option<Decimal>,
    pub usage_unit: Option<String>,
    pub cost_change_pct: Option<Decimal>,
    pub cost_change_amount: Option<Decimal>,
    pub anomaly_score: Option<Decimal>,
    pub is_anomaly: bool,
    pub tags_summary: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Domain model for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyCostAggregateModel {
    pub id: Uuid,
    pub account_id: String,
    pub service_name: String,
    pub month_year: String, // YYYY-MM format
    pub total_cost: f64,
    pub usage_amount: Option<f64>,
    pub usage_unit: Option<String>,
    pub cost_change_pct: Option<f64>,
    pub cost_change_amount: Option<f64>,
    pub anomaly_score: Option<f64>,
    pub is_anomaly: bool,
    pub tags_summary: Option<serde_json::Value>,
}

impl From<Model> for MonthlyCostAggregateModel {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            service_name: model.service_name,
            month_year: model.month_year.format("%Y-%m").to_string(),
            total_cost: model.total_cost.to_string().parse().unwrap_or(0.0),
            usage_amount: model
                .usage_amount
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            usage_unit: model.usage_unit,
            cost_change_pct: model
                .cost_change_pct
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            cost_change_amount: model
                .cost_change_amount
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            anomaly_score: model
                .anomaly_score
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            is_anomaly: model.is_anomaly,
            tags_summary: model.tags_summary.map(|j| j.into()),
        }
    }
}
