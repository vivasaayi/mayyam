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


use crate::models::cost_budget::{Budget, BudgetDto, BudgetType, BudgetStatus, BudgetAlert};
use crate::repositories::cost_budget_repository::CostBudgetRepository;
use sea_orm::{DatabaseConnection, ActiveModelTrait, Set};
use chrono::{Utc, NaiveDate};
use bigdecimal::ToPrimitive;
use uuid::Uuid;

#[derive(Clone)]
pub struct BudgetService {
    db: DatabaseConnection,
    repository: CostBudgetRepository,
}

impl BudgetService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            repository: CostBudgetRepository::new(db.clone()),
            db,
        }
    }

    /// Create a new budget
    pub async fn create_budget(&self, dto: BudgetDto) -> Result<Budget, String> {
        // Validate budget data
        self.validate_budget_dto(&dto).await?;

        let active_model = dto.into_active_model();
        let budget = active_model.insert(&self.db).await
            .map_err(|e| format!("Failed to create budget: {}", e))?;

        Ok(budget)
    }

    /// Get budget by ID
    pub async fn get_budget(&self, budget_id: Uuid) -> Result<Option<Budget>, String> {
        self.repository.find_by_id(budget_id).await
    }

    /// Get all budgets for an account
    pub async fn get_budgets_by_account(&self, account_id: &str) -> Result<Vec<Budget>, String> {
        self.repository.find_by_account_id(account_id).await
    }

    /// Update budget
    pub async fn update_budget(&self, budget_id: Uuid, dto: BudgetDto) -> Result<Budget, String> {
        let existing_budget = self.repository.find_by_id(budget_id).await?
            .ok_or_else(|| "Budget not found".to_string())?;

        // Validate updated budget data
        self.validate_budget_dto(&dto).await?;

        let mut active_model: crate::models::cost_budget::ActiveModel = existing_budget.into();

        // Update fields
        active_model.name = Set(dto.name);
        active_model.description = Set(dto.description);
        active_model.budget_type = Set(dto.budget_type.to_string());
        active_model.budget_period = Set(dto.budget_period.to_string());
        active_model.amount = Set(dto.amount);
        active_model.currency = Set(dto.currency);
        active_model.start_date = Set(dto.start_date);
        active_model.end_date = Set(dto.end_date);
        active_model.alert_thresholds = Set(serde_json::to_value(&dto.alert_thresholds).map_err(|e| format!("Invalid alert thresholds: {}", e))?);
        active_model.tags = Set(dto.tags);
        active_model.updated_at = Set(Utc::now().naive_utc());

        let updated_budget = active_model.update(&self.db).await
            .map_err(|e| format!("Failed to update budget: {}", e))?;

        Ok(updated_budget)
    }

    /// Delete budget
    pub async fn delete_budget(&self, budget_id: Uuid) -> Result<(), String> {
        self.repository.delete_by_id(budget_id).await
    }

    /// Get budget status with current spending
    pub async fn get_budget_status(&self, budget_id: Uuid) -> Result<BudgetStatus, String> {
        let budget = self.repository.find_by_id(budget_id).await?
            .ok_or_else(|| "Budget not found".to_string())?;

        // Calculate current spending based on budget type and period
        let current_spending = self.calculate_current_spending(&budget).await?;

        let status = self.calculate_budget_status(&budget, current_spending);

        Ok(status)
    }

    /// Get budget alerts for an account
    pub async fn get_budget_alerts(&self, account_id: &str) -> Result<Vec<BudgetAlert>, String> {
        let budgets = self.repository.find_by_account_id(account_id).await?;

        let mut alerts = Vec::new();

        for budget in budgets {
            let status = self.get_budget_status(budget.id).await?;
            let budget_alerts = self.generate_alerts(&budget, &status);
            alerts.extend(budget_alerts);
        }

        Ok(alerts)
    }

    /// Calculate current spending for a budget
    async fn calculate_current_spending(&self, budget: &Budget) -> Result<f64, String> {
        use crate::repositories::cost_analytics::CostAnalyticsRepository;
        use chrono::Datelike;

        let cost_repo = CostAnalyticsRepository::new(std::sync::Arc::new(self.db.clone()));

        let now = Utc::now().naive_utc();
        let (start_date, end_date) = self.get_budget_period_dates(budget, now.date());

        // Query cost data based on budget type
        let total_cost = match budget.budget_type.as_str() {
            "overall" => {
                let cost_data = cost_repo.get_cost_data_by_date_range(
                    &budget.account_id,
                    start_date,
                    end_date,
                    None,
                ).await.map_err(|e| format!("Failed to get cost data: {}", e))?;

                cost_data.iter().map(|data| data.unblended_cost.to_f64().unwrap_or(0.0)).sum()
            }
            "service" => {
                if let Some(service) = budget.tags.get("service").and_then(|v| v.as_str()) {
                    let cost_data = cost_repo.get_cost_data_by_date_range(
                        &budget.account_id,
                        start_date,
                        end_date,
                        Some(service.to_string()),
                    ).await.map_err(|e| format!("Failed to get cost data: {}", e))?;

                    cost_data.iter().map(|data| data.unblended_cost.to_f64().unwrap_or(0.0)).sum()
                } else {
                    return Err("Service budget missing service tag".to_string());
                }
            }
            "category" => {
                if let Some(category) = budget.tags.get("category").and_then(|v| v.as_str()) {
                    let cost_data = cost_repo.get_cost_data_by_date_range(
                        &budget.account_id,
                        start_date,
                        end_date,
                        None,
                    ).await.map_err(|e| format!("Failed to get cost data: {}", e))?;

                    // Filter by category (simplified - would need more sophisticated logic)
                    cost_data.iter()
                        .filter(|data| data.service_name.contains(category) || 
                                data.tags.as_ref()
                                    .and_then(|t| serde_json::to_string(t).ok())
                                    .map(|tag_str| tag_str.contains(category))
                                    .unwrap_or(false))
                        .map(|data| data.unblended_cost.to_f64().unwrap_or(0.0))
                        .sum()
                } else {
                    return Err("Category budget missing category tag".to_string());
                }
            }
            "tag_based" => {
                let tag_key = budget.tags.get("tag_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Tag-based budget missing tag_key".to_string())?;
                let tag_value = budget.tags.get("tag_value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Tag-based budget missing tag_value".to_string())?;

                let cost_data = cost_repo.get_cost_data_by_date_range(
                    &budget.account_id,
                    start_date,
                    end_date,
                    None,
                ).await.map_err(|e| format!("Failed to get cost data: {}", e))?;

                // Filter by tag (simplified)
                cost_data.iter()
                    .filter(|data| {
                        data.tags.as_ref()
                            .and_then(|t| serde_json::to_string(t).ok())
                            .map(|tag_str| tag_str.contains(&format!("\"{}\":\"{}\"", tag_key, tag_value)))
                            .unwrap_or(false)
                    })
                    .map(|data| data.unblended_cost.to_f64().unwrap_or(0.0))
                    .sum()
            }
            "resource" => {
                let resource_id = budget.tags.get("resource_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Resource budget missing resource_id".to_string())?;

                let cost_data = cost_repo.get_resource_costs(
                    &budget.account_id,
                    start_date,
                    end_date,
                    Some(resource_id),
                    None,
                    None,
                    None,
                    None,
                    None,
                ).await.map_err(|e| format!("Failed to get resource costs: {}", e))?;

                cost_data.iter().map(|data| data.unblended_cost.to_f64().unwrap_or(0.0)).sum()
            }
            _ => return Err(format!("Unknown budget type: {}", budget.budget_type)),
        };

        Ok(total_cost)
    }

    /// Calculate budget status
    fn calculate_budget_status(&self, budget: &Budget, current_spending: f64) -> BudgetStatus {
        let budget_amount = budget.amount;
        let utilization_percentage = if budget_amount > 0.0 {
            (current_spending / budget_amount) * 100.0
        } else {
            0.0
        };

        let forecasted_spending = self.calculate_forecasted_spending(budget, current_spending);

        BudgetStatus {
            budget_id: budget.id,
            current_spending,
            budget_amount,
            utilization_percentage,
            forecasted_spending,
            remaining_budget: budget_amount - current_spending,
            status: self.determine_budget_health_from_thresholds(budget, utilization_percentage),
        }
    }

    /// Generate alerts based on budget status
    fn generate_alerts(&self, budget: &Budget, status: &BudgetStatus) -> Vec<BudgetAlert> {
        let mut alerts = Vec::new();
        let utilization = status.utilization_percentage;

        // Deserialize alert thresholds from JSON
        let thresholds: Vec<crate::models::cost_budget::BudgetAlertThreshold> = serde_json::from_value(budget.alert_thresholds.clone())
            .unwrap_or_else(|_| vec![]);

        for threshold in thresholds {
            if utilization >= threshold.percentage {
                alerts.push(BudgetAlert {
                    id: Uuid::new_v4(),
                    budget_id: budget.id,
                    alert_type: threshold.alert_type.clone(),
                    threshold_percentage: threshold.percentage,
                    current_percentage: status.utilization_percentage,
                    message: format!(
                        "Budget '{}' has reached {:.1}% utilization (threshold: {:.1}%)",
                        budget.name, utilization, threshold.percentage
                    ),
                    created_at: Utc::now().naive_utc(),
                });
            }
        }

        alerts
    }

    /// Get period dates for budget calculation
    fn get_budget_period_dates(&self, budget: &Budget, current_date: NaiveDate) -> (NaiveDate, NaiveDate) {
        use chrono::Datelike;

        match budget.budget_period.as_str() {
            "monthly" => {
                let start = NaiveDate::from_ymd_opt(current_date.year(), current_date.month(), 1).unwrap();
                let end = if current_date.month() == 12 {
                    NaiveDate::from_ymd_opt(current_date.year() + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(current_date.year(), current_date.month() + 1, 1).unwrap()
                }.pred_opt().unwrap();
                (start, end)
            }
            "quarterly" => {
                let quarter = (current_date.month() - 1) / 3 + 1;
                let start_month = (quarter - 1) * 3 + 1;
                let start = NaiveDate::from_ymd_opt(current_date.year(), start_month, 1).unwrap();
                let end_month = quarter * 3;
                let end = NaiveDate::from_ymd_opt(current_date.year(), end_month + 1, 1).unwrap().pred_opt().unwrap();
                (start, end)
            }
            "yearly" => {
                let start = NaiveDate::from_ymd_opt(current_date.year(), 1, 1).unwrap();
                let end = NaiveDate::from_ymd_opt(current_date.year(), 12, 31).unwrap();
                (start, end)
            }
            "custom" => {
                (budget.start_date, budget.end_date.unwrap_or(budget.start_date))
            }
            _ => {
                // Default to monthly
                let start = NaiveDate::from_ymd_opt(current_date.year(), current_date.month(), 1).unwrap();
                let end = if current_date.month() == 12 {
                    NaiveDate::from_ymd_opt(current_date.year() + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(current_date.year(), current_date.month() + 1, 1).unwrap()
                }.pred_opt().unwrap();
                (start, end)
            }
        }
    }

    /// Calculate forecasted spending (simple linear projection)
    fn calculate_forecasted_spending(&self, budget: &Budget, current_spending: f64) -> f64 {
        // Simple forecasting: assume linear spending throughout the period
        // In a real implementation, this would use more sophisticated forecasting
        let now = Utc::now().naive_utc().date();
        let (start_date, end_date) = self.get_budget_period_dates(budget, now);

        let total_days = (end_date - start_date).num_days() as f64;
        let elapsed_days = (now - start_date).num_days() as f64;

        if total_days > 0.0 && elapsed_days > 0.0 {
            let daily_rate = current_spending / elapsed_days;
            daily_rate * total_days
        } else {
            current_spending
        }
    }

    /// Determine budget health status
    fn determine_budget_health(&self, thresholds: &[crate::models::cost_budget::BudgetAlertThreshold], utilization: f64) -> String {
        // Sort thresholds by percentage descending
        let mut sorted_thresholds = thresholds.to_vec();
        sorted_thresholds.sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap());

        for threshold in sorted_thresholds {
            if utilization >= threshold.percentage {
                return match threshold.alert_type.as_str() {
                    "warning" => "warning".to_string(),
                    "critical" => "critical".to_string(),
                    _ => "normal".to_string(),
                };
            }
        }

        "normal".to_string()
    }

    /// Determine budget health status
    fn determine_budget_health_from_thresholds(&self, budget: &Budget, utilization: f64) -> String {
        // Deserialize alert thresholds from JSON
        let thresholds: Vec<crate::models::cost_budget::BudgetAlertThreshold> = serde_json::from_value(budget.alert_thresholds.clone())
            .unwrap_or_else(|_| vec![]);

        self.determine_budget_health(&thresholds, utilization)
    }

    /// Validate budget DTO
    async fn validate_budget_dto(&self, dto: &BudgetDto) -> Result<(), String> {
        if dto.name.trim().is_empty() {
            return Err("Budget name cannot be empty".to_string());
        }

        if dto.amount <= 0.0 {
            return Err("Budget amount must be greater than 0".to_string());
        }

        if dto.start_date >= dto.end_date.unwrap_or(NaiveDate::from_ymd_opt(9999, 12, 31).unwrap()) {
            return Err("Start date must be before end date".to_string());
        }

        // Validate budget type specific requirements
        match dto.budget_type {
            BudgetType::Service => {
                if !dto.tags.get("service").is_some() {
                    return Err("Service budget must specify a service tag".to_string());
                }
            }
            BudgetType::Category => {
                if !dto.tags.get("category").is_some() {
                    return Err("Category budget must specify a category tag".to_string());
                }
            }
            BudgetType::TagBased => {
                if !dto.tags.get("tag_key").is_some() || !dto.tags.get("tag_value").is_some() {
                    return Err("Tag-based budget must specify tag_key and tag_value".to_string());
                }
            }
            _ => {}
        }

        Ok(())
    }
}