use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "aws_cost_anomalies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub account_id: String,
    pub service_name: String,
    pub anomaly_type: String,
    pub severity: String,
    pub detected_date: Date,
    pub anomaly_score: Decimal,
    pub baseline_cost: Option<Decimal>,
    pub actual_cost: Decimal,
    pub cost_difference: Option<Decimal>,
    pub percentage_change: Option<Decimal>,
    pub description: Option<String>,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Domain model for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAnomalyModel {
    pub id: Uuid,
    pub account_id: String,
    pub service_name: String,
    pub anomaly_type: String,
    pub severity: String,
    pub detected_date: String, // ISO date format
    pub anomaly_score: f64,
    pub baseline_cost: Option<f64>,
    pub actual_cost: f64,
    pub cost_difference: Option<f64>,
    pub percentage_change: Option<f64>,
    pub description: Option<String>,
    pub status: String,
}

impl From<Model> for CostAnomalyModel {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            service_name: model.service_name,
            anomaly_type: model.anomaly_type,
            severity: model.severity,
            detected_date: model.detected_date.to_string(),
            anomaly_score: model.anomaly_score.to_string().parse().unwrap_or(0.0),
            baseline_cost: model
                .baseline_cost
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            actual_cost: model.actual_cost.to_string().parse().unwrap_or(0.0),
            cost_difference: model
                .cost_difference
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            percentage_change: model
                .percentage_change
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            description: model.description,
            status: model.status,
        }
    }
}
