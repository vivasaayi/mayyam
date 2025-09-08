use std::sync::Arc;
use sea_orm::*;
use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use crate::models::aws_cost_data::{Entity as CostData, Model as CostDataModel};
use crate::models::aws_monthly_cost_aggregates::{Entity as MonthlyCostAggregates, Model as MonthlyCostAggregateModel};
use crate::models::aws_cost_anomalies::{Entity as CostAnomalies, Model as CostAnomalyModel};
use crate::models::aws_cost_insights::{Entity as CostInsights, Model as CostInsightModel};
use crate::errors::AppError;

#[derive(Debug)]
pub struct CostAnalyticsRepository {
    db: Arc<DatabaseConnection>,
}

impl CostAnalyticsRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    // Cost Data operations
    pub async fn insert_cost_data(&self, cost_data: Vec<crate::models::aws_cost_data::ActiveModel>) -> Result<(), AppError> {
        CostData::insert_many(cost_data)
            .exec(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;
        
        Ok(())
    }

    pub async fn get_cost_data_by_date_range(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        service_name: Option<String>,
    ) -> Result<Vec<CostDataModel>, AppError> {
        let mut query = CostData::find()
            .filter(crate::models::aws_cost_data::Column::AccountId.eq(account_id))
            .filter(crate::models::aws_cost_data::Column::UsageStart.gte(start_date))
            .filter(crate::models::aws_cost_data::Column::UsageEnd.lte(end_date));

        if let Some(service) = service_name {
            query = query.filter(crate::models::aws_cost_data::Column::ServiceName.eq(service));
        }

        let results = query
            .order_by_desc(crate::models::aws_cost_data::Column::UsageStart)
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(results)
    }

    // Monthly Aggregates operations
    pub async fn insert_monthly_aggregate(&self, aggregate: crate::models::aws_monthly_cost_aggregates::ActiveModel) -> Result<MonthlyCostAggregateModel, AppError> {
        let result = MonthlyCostAggregates::insert(aggregate)
            .exec_with_returning(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;
        
        Ok(result)
    }

    pub async fn get_monthly_aggregates_by_account(
        &self,
        account_id: &str,
        months: Option<i32>,
    ) -> Result<Vec<MonthlyCostAggregateModel>, AppError> {
        let mut query = MonthlyCostAggregates::find()
            .filter(crate::models::aws_monthly_cost_aggregates::Column::AccountId.eq(account_id));

        if let Some(month_limit) = months {
            let cutoff_date = Utc::now().naive_utc().date() - chrono::Duration::days(month_limit as i64 * 30);
            query = query.filter(crate::models::aws_monthly_cost_aggregates::Column::MonthYear.gte(cutoff_date));
        }

        let results = query
            .order_by_desc(crate::models::aws_monthly_cost_aggregates::Column::MonthYear)
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(results)
    }

    pub async fn get_top_services_by_cost(
        &self,
        account_id: &str,
        month_year: NaiveDate,
        limit: Option<u64>,
    ) -> Result<Vec<MonthlyCostAggregateModel>, AppError> {
        let query = MonthlyCostAggregates::find()
            .filter(crate::models::aws_monthly_cost_aggregates::Column::AccountId.eq(account_id))
            .filter(crate::models::aws_monthly_cost_aggregates::Column::MonthYear.eq(month_year))
            .order_by_desc(crate::models::aws_monthly_cost_aggregates::Column::TotalCost)
            .limit(limit.unwrap_or(10));

        let results = query
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(results)
    }

    pub async fn get_anomalies(&self) -> Result<Vec<MonthlyCostAggregateModel>, AppError> {
        let results = MonthlyCostAggregates::find()
            .filter(crate::models::aws_monthly_cost_aggregates::Column::IsAnomaly.eq(true))
            .order_by_desc(crate::models::aws_monthly_cost_aggregates::Column::MonthYear)
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(results)
    }

    // Cost Anomalies operations
    pub async fn insert_cost_anomaly(&self, anomaly: crate::models::aws_cost_anomalies::ActiveModel) -> Result<CostAnomalyModel, AppError> {
        let result = CostAnomalies::insert(anomaly)
            .exec_with_returning(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;
        
        Ok(result)
    }

    pub async fn get_cost_anomalies_by_account(
        &self,
        account_id: &str,
        severity: Option<String>,
        status: Option<String>,
    ) -> Result<Vec<CostAnomalyModel>, AppError> {
        let mut query = CostAnomalies::find()
            .filter(crate::models::aws_cost_anomalies::Column::AccountId.eq(account_id));

        if let Some(sev) = severity {
            query = query.filter(crate::models::aws_cost_anomalies::Column::Severity.eq(sev));
        }

        if let Some(stat) = status {
            query = query.filter(crate::models::aws_cost_anomalies::Column::Status.eq(stat));
        }

        let results = query
            .order_by_desc(crate::models::aws_cost_anomalies::Column::DetectedDate)
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(results)
    }

    // Cost Insights operations
    pub async fn insert_cost_insight(&self, insight: crate::models::aws_cost_insights::ActiveModel) -> Result<CostInsightModel, AppError> {
        let result = CostInsights::insert(insight)
            .exec_with_returning(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;
        
        Ok(result)
    }

    pub async fn get_cost_insights_by_anomaly(&self, anomaly_id: Uuid) -> Result<Vec<CostInsightModel>, AppError> {
        let results = CostInsights::find()
            .filter(crate::models::aws_cost_insights::Column::AnomalyId.eq(anomaly_id))
            .order_by_desc(crate::models::aws_cost_insights::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(results)
    }

    pub async fn get_recent_insights(
        &self,
        account_id: &str,
        insight_type: Option<String>,
        limit: Option<u64>,
    ) -> Result<Vec<CostInsightModel>, AppError> {
        let mut query = CostInsights::find()
            .filter(crate::models::aws_cost_insights::Column::AccountId.eq(account_id));

        if let Some(insight_t) = insight_type {
            query = query.filter(crate::models::aws_cost_insights::Column::InsightType.eq(insight_t));
        }

        let results = query
            .order_by_desc(crate::models::aws_cost_insights::Column::CreatedAt)
            .limit(limit.unwrap_or(20))
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(results)
    }

    // Analytics queries
    pub async fn get_cost_trend_by_service(
        &self,
        account_id: &str,
        service_name: &str,
        months: i32,
    ) -> Result<Vec<MonthlyCostAggregateModel>, AppError> {
        let cutoff_date = Utc::now().naive_utc().date() - chrono::Duration::days(months as i64 * 30);
        
        let results = MonthlyCostAggregates::find()
            .filter(crate::models::aws_monthly_cost_aggregates::Column::AccountId.eq(account_id))
            .filter(crate::models::aws_monthly_cost_aggregates::Column::ServiceName.eq(service_name))
            .filter(crate::models::aws_monthly_cost_aggregates::Column::MonthYear.gte(cutoff_date))
            .order_by_asc(crate::models::aws_monthly_cost_aggregates::Column::MonthYear)
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(results)
    }

    pub async fn calculate_monthly_totals(&self, account_id: &str) -> Result<Vec<(String, f64)>, AppError> {
        let query = format!(
            r#"
            SELECT 
                TO_CHAR(month_year, 'YYYY-MM') as month,
                SUM(total_cost) as total
            FROM aws_monthly_cost_aggregates 
            WHERE account_id = '{}' 
            GROUP BY month_year, TO_CHAR(month_year, 'YYYY-MM')
            ORDER BY month_year DESC
            LIMIT 12
            "#,
            account_id
        );

        let statement = Statement::from_string(DatabaseBackend::Postgres, query);
        let query_result = self.db.query_all(statement).await
            .map_err(|e| AppError::Database(e))?;

        let mut results = Vec::new();
        for row in query_result {
            let month: String = row.try_get("", "month")
                .map_err(|e| AppError::Database(e))?;
            let total: sea_orm::prelude::Decimal = row.try_get("", "total")
                .map_err(|e| AppError::Database(e))?;
            
            results.push((month, total.to_string().parse().unwrap_or(0.0)));
        }

        Ok(results)
    }
}
