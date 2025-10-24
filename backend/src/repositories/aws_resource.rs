use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    ModelTrait, Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Set,
};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use crate::config::Config;
use crate::errors::AppError;
use crate::models::aws_resource::{
    self, ActiveModel, AwsResourceDto, AwsResourcePage, AwsResourceQuery, Entity as AwsResource,
    Model,
};

#[derive(Debug)]
pub struct AwsResourceRepository {
    db: Arc<DatabaseConnection>,
    config: Config,
}

impl AwsResourceRepository {
    pub fn new(db: Arc<DatabaseConnection>, config: Config) -> Self {
        Self { db, config }
    }

    // Create a new AWS resource entry
    pub async fn create(&self, resource: &AwsResourceDto) -> Result<Model, AppError> {
        let now = Utc::now();

        let active_model = ActiveModel {
            id: Set(Uuid::new_v4()),
            sync_id: Set(resource.sync_id),
            account_id: Set(resource.account_id.clone()),
            profile: Set(resource.profile.clone()),
            region: Set(resource.region.clone()),
            resource_type: Set(resource.resource_type.clone()),
            resource_id: Set(resource.resource_id.clone()),
            arn: Set(resource.arn.clone()),
            name: Set(resource.name.clone()),
            tags: Set(resource.tags.clone()),
            resource_data: Set(resource.resource_data.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            last_refreshed: Set(now),
        };

        let model = active_model
            .insert(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        info!("AWS resource created: {}", model.id);
        Ok(model)
    }

    // Update an existing AWS resource
    pub async fn update(&self, id: Uuid, resource: &AwsResourceDto) -> Result<Model, AppError> {
        let aws_resource = AwsResource::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?
            .ok_or_else(|| AppError::NotFound(format!("AWS resource with ID {} not found", id)))?;

        let now = Utc::now();
        let mut active_model = aws_resource.into_active_model();

        active_model.account_id = Set(resource.account_id.clone());
        active_model.profile = Set(resource.profile.clone());
        active_model.region = Set(resource.region.clone());
        active_model.resource_type = Set(resource.resource_type.clone());
        active_model.resource_id = Set(resource.resource_id.clone());
        active_model.arn = Set(resource.arn.clone());
        active_model.name = Set(resource.name.clone());
        active_model.tags = Set(resource.tags.clone());
        active_model.resource_data = Set(resource.resource_data.clone());
        active_model.updated_at = Set(now);
        active_model.last_refreshed = Set(now);

        let updated_model = active_model
            .update(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        info!("AWS resource updated: {}", updated_model.id);
        Ok(updated_model)
    }

    // Find resource by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Model>, AppError> {
        let resource = AwsResource::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(resource)
    }

    // Find resource by ARN
    pub async fn find_by_arn(&self, arn: &str) -> Result<Option<Model>, AppError> {
        let resource = AwsResource::find()
            .filter(aws_resource::Column::Arn.eq(arn))
            .one(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(resource)
    }

    // Find resources by account ID and type
    pub async fn find_by_account_and_type(
        &self,
        account_id: &str,
        resource_type: &str,
    ) -> Result<Vec<Model>, AppError> {
        let resources = AwsResource::find()
            .filter(
                Condition::all()
                    .add(aws_resource::Column::AccountId.eq(account_id))
                    .add(aws_resource::Column::ResourceType.eq(resource_type)),
            )
            .order_by(aws_resource::Column::Name, Order::Asc)
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(resources)
    }

    // Search resources with pagination
    pub async fn search(&self, query: &AwsResourceQuery) -> Result<AwsResourcePage, AppError> {
        let page = query.page.unwrap_or(0);
        let page_size = query.page_size.unwrap_or(10);

        let mut condition = Condition::all();

        if let Some(account_id) = &query.account_id {
            condition = condition.add(aws_resource::Column::AccountId.eq(account_id.clone()));
        }

        if let Some(profile) = &query.profile {
            condition = condition.add(aws_resource::Column::Profile.eq(profile.clone()));
        }

        if let Some(region) = &query.region {
            condition = condition.add(aws_resource::Column::Region.eq(region.clone()));
        }

        if let Some(resource_type) = &query.resource_type {
            condition = condition.add(aws_resource::Column::ResourceType.eq(resource_type.clone()));
        }

        if let Some(resource_id) = &query.resource_id {
            condition = condition.add(aws_resource::Column::ResourceId.eq(resource_id.clone()));
        }

        if let Some(sync_id) = &query.sync_id {
            condition = condition.add(aws_resource::Column::SyncId.eq(*sync_id));
        }

        if let Some(name) = &query.name {
            condition = condition.add(aws_resource::Column::Name.like(format!("%{}%", name)));
        }

        // Count total results first
        let total = AwsResource::find()
            .filter(condition.clone())
            .count(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        // Then fetch the requested page
        let resources = AwsResource::find()
            .filter(condition)
            .order_by(aws_resource::Column::UpdatedAt, Order::Desc)
            .limit(Some(page_size))
            .offset(Some(page * page_size))
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        let total_pages = (total as f64 / page_size as f64).ceil() as u64;

        Ok(AwsResourcePage {
            resources,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    // Delete a resource
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let res = AwsResource::delete_by_id(id)
            .exec(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        if res.rows_affected == 0 {
            return Err(AppError::NotFound(format!(
                "AWS resource with ID {} not found",
                id
            )));
        }

        info!("AWS resource deleted: {}", id);
        Ok(())
    }

    // Update resource data only
    pub async fn update_resource_data(
        &self,
        id: Uuid,
        resource_data: serde_json::Value,
    ) -> Result<Model, AppError> {
        let aws_resource = AwsResource::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?
            .ok_or_else(|| AppError::NotFound(format!("AWS resource with ID {} not found", id)))?;

        let now = Utc::now();
        let mut active_model = aws_resource.into_active_model();

        active_model.resource_data = Set(resource_data);
        active_model.updated_at = Set(now);
        active_model.last_refreshed = Set(now);

        let updated_model = active_model
            .update(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        info!("AWS resource data updated: {}", updated_model.id);
        Ok(updated_model)
    }

    // Find newly added resources within a time window
    pub async fn find_newly_added_resources(
        &self,
        account_id: Option<String>,
        days_back: i64,
        limit: Option<u64>,
    ) -> Result<Vec<Model>, AppError> {
        use chrono::{Duration, Utc};

        let cutoff_date = Utc::now() - Duration::days(days_back);

        let mut query = AwsResource::find()
            .filter(aws_resource::Column::CreatedAt.gte(cutoff_date))
            .order_by(aws_resource::Column::CreatedAt, Order::Desc);

        if let Some(account) = account_id {
            query = query.filter(aws_resource::Column::AccountId.eq(account));
        }

        if let Some(limit_val) = limit {
            query = query.limit(Some(limit_val));
        }

        let resources = query
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        Ok(resources)
    }

    // Get resource count changes over time
    pub async fn get_resource_count_changes(
        &self,
        account_id: Option<String>,
        days_back: i64,
    ) -> Result<Vec<(String, i64)>, AppError> {
        use chrono::{Duration, NaiveDate, Utc};

        let cutoff_date = Utc::now() - Duration::days(days_back);

        // This is a simplified version - in a real implementation you'd want more sophisticated
        // time-series analysis. For now, we'll return daily counts.
        let query = format!(
            r#"
            SELECT
                DATE(created_at) as date,
                COUNT(*) as count
            FROM aws_resources
            WHERE created_at >= '{}'
            {}
            GROUP BY DATE(created_at)
            ORDER BY date DESC
            "#,
            cutoff_date.format("%Y-%m-%d %H:%M:%S%.3f%z"),
            account_id
                .map(|id| format!("AND account_id = '{}'", id))
                .unwrap_or_default()
        );

        let statement = sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            query,
        );

        let query_result = self
            .db
            .query_all(statement)
            .await
            .map_err(|e| AppError::Database(e))?;

        let mut results = Vec::new();
        for row in query_result {
            let date: String = row
                .try_get("", "date")
                .map_err(|e| AppError::Database(e))?;
            let count: i64 = row
                .try_get("", "count")
                .map_err(|e| AppError::Database(e))?;

            results.push((date, count));
        }

        Ok(results)
    }

    // Detect cost increases by comparing current vs historical spending
    pub async fn detect_cost_increases(
        &self,
        account_id: Option<String>,
        days_back: i64,
        threshold_percentage: f64,
        min_cost_threshold: f64,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        use chrono::{Duration, Utc};

        let cutoff_date = Utc::now() - Duration::days(days_back);

        // This query will need to be adapted based on your actual cost data table structure
        // For now, we'll assume you have a cost_data table with resource_id, date, and amount
        let query = format!(
            r#"
            WITH recent_costs AS (
                SELECT
                    resource_id,
                    SUM(amount) as recent_total,
                    AVG(amount) as recent_avg
                FROM cost_data
                WHERE date >= '{}'
                {}
                GROUP BY resource_id
            ),
            historical_costs AS (
                SELECT
                    resource_id,
                    AVG(amount) as historical_avg
                FROM cost_data
                WHERE date < '{}' AND date >= '{}'
                {}
                GROUP BY resource_id
            )
            SELECT
                r.resource_id,
                r.name,
                r.resource_type,
                r.region,
                COALESCE(rc.recent_total, 0) as recent_total,
                COALESCE(hc.historical_avg, 0) as historical_avg,
                CASE
                    WHEN COALESCE(hc.historical_avg, 0) > 0 THEN
                        ((COALESCE(rc.recent_total, 0) - COALESCE(hc.historical_avg, 0)) / COALESCE(hc.historical_avg, 0)) * 100
                    ELSE 0
                END as percentage_increase
            FROM aws_resources r
            LEFT JOIN recent_costs rc ON r.resource_id = rc.resource_id
            LEFT JOIN historical_costs hc ON r.resource_id = hc.resource_id
            WHERE COALESCE(rc.recent_total, 0) >= {}
                AND (
                    CASE
                        WHEN COALESCE(hc.historical_avg, 0) > 0 THEN
                            ((COALESCE(rc.recent_total, 0) - COALESCE(hc.historical_avg, 0)) / COALESCE(hc.historical_avg, 0)) * 100
                        ELSE 0
                    END
                ) >= {}
            ORDER BY percentage_increase DESC
            "#,
            cutoff_date.format("%Y-%m-%d"),
            account_id
                .as_ref()
                .map(|id| format!("AND account_id = '{}'", id))
                .unwrap_or_default(),
            cutoff_date.format("%Y-%m-%d"),
            (Utc::now() - Duration::days(days_back * 2)).format("%Y-%m-%d"),
            account_id
                .as_ref()
                .map(|id| format!("AND account_id = '{}'", id))
                .unwrap_or_default(),
            min_cost_threshold,
            threshold_percentage
        );

        let statement = sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            query,
        );

        let query_result = self
            .db
            .query_all(statement)
            .await
            .map_err(|e| AppError::Database(e))?;

        let mut results = Vec::new();
        for row in query_result {
            let result = serde_json::json!({
                "resource_id": row.try_get::<String>("", "resource_id").unwrap_or_default(),
                "name": row.try_get::<String>("", "name").unwrap_or_default(),
                "resource_type": row.try_get::<String>("", "resource_type").unwrap_or_default(),
                "region": row.try_get::<String>("", "region").unwrap_or_default(),
                "recent_total": row.try_get::<f64>("", "recent_total").unwrap_or(0.0),
                "historical_avg": row.try_get::<f64>("", "historical_avg").unwrap_or(0.0),
                "percentage_increase": row.try_get::<f64>("", "percentage_increase").unwrap_or(0.0)
            });
            results.push(result);
        }

        Ok(results)
    }

    // Get historical cost data for a specific resource with time-series formatting
    pub async fn get_resource_cost_history(
        &self,
        resource_id: &str,
        account_id: Option<String>,
        days_back: i64,
        granularity: String, // "daily" or "weekly"
    ) -> Result<Vec<serde_json::Value>, AppError> {
        use chrono::{Duration, NaiveDate, Utc};

        let cutoff_date = Utc::now() - Duration::days(days_back);

        let group_by_clause = match granularity.as_str() {
            "weekly" => "DATE_TRUNC('week', date)",
            _ => "date", // daily by default
        };

        let query = format!(
            r#"
            SELECT
                {} as period,
                SUM(amount) as total_cost,
                AVG(amount) as avg_daily_cost,
                COUNT(*) as data_points
            FROM cost_data
            WHERE resource_id = '{}'
                AND date >= '{}'
                {}
            GROUP BY {}
            ORDER BY period ASC
            "#,
            group_by_clause,
            resource_id,
            cutoff_date.format("%Y-%m-%d"),
            account_id
                .as_ref()
                .map(|id| format!("AND account_id = '{}'", id))
                .unwrap_or_default(),
            group_by_clause
        );

        let statement = sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            query,
        );

        let query_result = self
            .db
            .query_all(statement)
            .await
            .map_err(|e| AppError::Database(e))?;

        let mut results = Vec::new();
        for row in query_result {
            let period: String = row
                .try_get::<String>("", "period")
                .unwrap_or_default();
            let total_cost: f64 = row
                .try_get::<f64>("", "total_cost")
                .unwrap_or(0.0);
            let avg_daily_cost: f64 = row
                .try_get::<f64>("", "avg_daily_cost")
                .unwrap_or(0.0);
            let data_points: i64 = row
                .try_get::<i64>("", "data_points")
                .unwrap_or(0);

            let result = serde_json::json!({
                "period": period,
                "total_cost": total_cost,
                "avg_daily_cost": avg_daily_cost,
                "data_points": data_points,
                "resource_id": resource_id
            });
            results.push(result);
        }

        Ok(results)
    }
}
