use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "aws_cost_data")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub account_id: String,
    pub service_name: String,
    pub usage_type: Option<String>,
    pub operation: Option<String>,
    pub region: Option<String>,
    pub usage_start: Date,
    pub usage_end: Date,
    pub unblended_cost: Decimal,
    pub blended_cost: Decimal,
    pub usage_amount: Option<Decimal>,
    pub usage_unit: Option<String>,
    pub currency: String,
    pub tags: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Domain model for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostDataModel {
    pub id: Uuid,
    pub account_id: String,
    pub service_name: String,
    pub usage_type: Option<String>,
    pub operation: Option<String>,
    pub region: Option<String>,
    pub usage_start: String, // ISO date format
    pub usage_end: String,   // ISO date format
    pub unblended_cost: f64,
    pub blended_cost: f64,
    pub usage_amount: Option<f64>,
    pub usage_unit: Option<String>,
    pub currency: String,
    pub tags: Option<serde_json::Value>,
}

impl From<Model> for CostDataModel {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            service_name: model.service_name,
            usage_type: model.usage_type,
            operation: model.operation,
            region: model.region,
            usage_start: model.usage_start.to_string(),
            usage_end: model.usage_end.to_string(),
            unblended_cost: model.unblended_cost.to_string().parse().unwrap_or(0.0),
            blended_cost: model.blended_cost.to_string().parse().unwrap_or(0.0),
            usage_amount: model
                .usage_amount
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            usage_unit: model.usage_unit,
            currency: model.currency,
            tags: model.tags.map(|j| j.into()),
        }
    }
}
