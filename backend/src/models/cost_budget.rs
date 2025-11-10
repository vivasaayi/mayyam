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


use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use chrono::{NaiveDate, NaiveDateTime};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cost_budgets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub account_id: String,
    pub name: String,
    pub description: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub budget_type: String, // Overall, Service, Category, Tag-based
    #[sea_orm(column_type = "Text")]
    pub budget_period: String, // Monthly, Quarterly, Yearly, Custom
    pub amount: f64, // Budget amount
    pub currency: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub alert_thresholds: Json, // JSON array of alert thresholds
    pub tags: Json, // JSON object for additional metadata
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetDto {
    pub account_id: String,
    pub name: String,
    pub description: Option<String>,
    pub budget_type: BudgetType,
    pub budget_period: BudgetPeriod,
    pub amount: f64,
    pub currency: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub alert_thresholds: Vec<BudgetAlertThreshold>,
    pub tags: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlertThreshold {
    pub percentage: f64,
    pub alert_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub budget_id: Uuid,
    pub current_spending: f64,
    pub budget_amount: f64,
    pub utilization_percentage: f64,
    pub forecasted_spending: f64,
    pub remaining_budget: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    pub id: Uuid,
    pub budget_id: Uuid,
    pub alert_type: String,
    pub threshold_percentage: f64,
    pub current_percentage: f64,
    pub message: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetType {
    Overall,      // Total account budget
    Service,      // Budget for specific AWS service
    Category,     // Budget for cost category
    TagBased,     // Budget based on tags
    Resource,     // Budget for specific resource
}

impl std::fmt::Display for BudgetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetType::Overall => write!(f, "overall"),
            BudgetType::Service => write!(f, "service"),
            BudgetType::Category => write!(f, "category"),
            BudgetType::TagBased => write!(f, "tag_based"),
            BudgetType::Resource => write!(f, "resource"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    Monthly,
    Quarterly,
    Yearly,
    Custom, // For budgets with custom date ranges
}

impl std::fmt::Display for BudgetPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetPeriod::Monthly => write!(f, "monthly"),
            BudgetPeriod::Quarterly => write!(f, "quarterly"),
            BudgetPeriod::Yearly => write!(f, "yearly"),
            BudgetPeriod::Custom => write!(f, "custom"),
        }
    }
}

impl BudgetDto {
    pub fn into_active_model(self) -> ActiveModel {
        let alert_thresholds_json = serde_json::to_value(&self.alert_thresholds)
            .unwrap_or(serde_json::Value::Array(vec![]));

        ActiveModel {
            id: Set(Uuid::new_v4()),
            account_id: Set(self.account_id),
            name: Set(self.name),
            description: Set(self.description),
            budget_type: Set(self.budget_type.to_string()),
            budget_period: Set(self.budget_period.to_string()),
            amount: Set(self.amount),
            currency: Set(self.currency),
            start_date: Set(self.start_date),
            end_date: Set(self.end_date),
            alert_thresholds: Set(alert_thresholds_json),
            tags: Set(self.tags),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
        }
    }
}

pub type Budget = Model;