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


use crate::models::cost_budget::{Budget, Entity as BudgetEntity, Column as BudgetColumn};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait};
use uuid::Uuid;

#[derive(Clone)]
pub struct CostBudgetRepository {
    db: DatabaseConnection,
}

impl CostBudgetRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Find budget by ID
    pub async fn find_by_id(&self, budget_id: Uuid) -> Result<Option<Budget>, String> {
        BudgetEntity::find_by_id(budget_id)
            .one(&self.db)
            .await
            .map_err(|e| format!("Failed to find budget by ID: {}", e))
    }

    /// Find all budgets for an account
    pub async fn find_by_account_id(&self, account_id: &str) -> Result<Vec<Budget>, String> {
        BudgetEntity::find()
            .filter(BudgetColumn::AccountId.eq(account_id))
            .all(&self.db)
            .await
            .map_err(|e| format!("Failed to find budgets by account ID: {}", e))
    }

    /// Find budgets by type
    pub async fn find_by_type(&self, account_id: &str, budget_type: &str) -> Result<Vec<Budget>, String> {
        BudgetEntity::find()
            .filter(BudgetColumn::AccountId.eq(account_id))
            .filter(BudgetColumn::BudgetType.eq(budget_type))
            .all(&self.db)
            .await
            .map_err(|e| format!("Failed to find budgets by type: {}", e))
    }

    /// Create a new budget
    pub async fn create(&self, budget: Budget) -> Result<Budget, String> {
        let active_model: crate::models::cost_budget::ActiveModel = budget.into();
        active_model.insert(&self.db)
            .await
            .map_err(|e| format!("Failed to create budget: {}", e))
    }

    /// Update an existing budget
    pub async fn update(&self, budget: Budget) -> Result<Budget, String> {
        let active_model: crate::models::cost_budget::ActiveModel = budget.into();
        active_model.update(&self.db)
            .await
            .map_err(|e| format!("Failed to update budget: {}", e))
    }

    /// Delete budget by ID
    pub async fn delete_by_id(&self, budget_id: Uuid) -> Result<(), String> {
        BudgetEntity::delete_by_id(budget_id)
            .exec(&self.db)
            .await
            .map_err(|e| format!("Failed to delete budget: {}", e))?;

        Ok(())
    }

    /// Find budgets that are close to exceeding thresholds
    pub async fn find_budgets_near_threshold(&self, account_id: &str, _threshold_percentage: f64) -> Result<Vec<Budget>, String> {
        // This would require calculating current spending vs budget
        // For now, return all budgets - the service layer will filter
        BudgetEntity::find()
            .filter(BudgetColumn::AccountId.eq(account_id))
            .all(&self.db)
            .await
            .map_err(|e| format!("Failed to find budgets near threshold: {}", e))
    }

    /// Get budget count for an account
    pub async fn count_by_account(&self, account_id: &str) -> Result<u64, String> {
        let budgets = BudgetEntity::find()
            .filter(BudgetColumn::AccountId.eq(account_id))
            .all(&self.db)
            .await
            .map_err(|e| format!("Failed to count budgets: {}", e))?;

        Ok(budgets.len() as u64)
    }
}