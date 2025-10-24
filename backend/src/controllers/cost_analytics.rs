use actix_web::{web, HttpResponse, Result as ActixResult};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::repositories::aws_account::AwsAccountRepository;
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::repositories::cost_analytics::CostAnalyticsRepository;
use crate::services::aws_cost_analytics::{AwsCostAnalyticsService, CostAnalysisRequest};

#[derive(Debug, Deserialize)]
pub struct CostAnalysisQuery {
    pub account_id: String,
    pub start_date: String,             // YYYY-MM-DD format
    pub end_date: String,               // YYYY-MM-DD format
    pub service_filter: Option<String>, // Comma-separated service names
    pub granularity: Option<String>,    // "DAILY" or "MONTHLY", default "MONTHLY"
}

#[derive(Debug, Deserialize)]
pub struct MonthlyAggregatesQuery {
    pub account_id: String,
    pub months: Option<i32>, // Number of months to look back
}

#[derive(Debug, Deserialize)]
pub struct TopServicesQuery {
    pub account_id: String,
    pub month_year: String, // YYYY-MM format
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AnomaliesQuery {
    pub account_id: String,
    pub severity: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResourceCostQuery {
    pub account_id: String,
    pub resource_id: Option<String>,
    pub service_name: Option<String>,
    pub region: Option<String>,
    pub availability_zone: Option<String>,
    pub instance_type: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct InsightsQuery {
    pub account_id: String,
    pub insight_type: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct NewResourcesQuery {
    pub account_id: Option<String>,
    pub days_back: Option<i64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CostIncreaseQuery {
    pub account_id: Option<String>,
    pub days_back: Option<i64>,
    pub threshold_percentage: Option<f64>,
    pub min_cost_threshold: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct CostAnalysisResponse {
    pub success: bool,
    pub data: serde_json::Value,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    pub message: String,
}

impl From<AppError> for ErrorResponse {
    fn from(err: AppError) -> Self {
        Self {
            success: false,
            error: err.error_type().to_string(),
            message: err.to_string(),
        }
    }
}

/// Fetch real-time cost data from AWS Cost Explorer
pub async fn fetch_cost_data(
    cost_service: web::Data<Arc<AwsCostAnalyticsService>>,
    aws_account_repo: web::Data<Arc<AwsAccountRepository>>,
    query: web::Query<CostAnalysisQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!("Fetching cost data for account {}", query.account_id);

    // Parse dates
    let start_date = NaiveDate::parse_from_str(&query.start_date, "%Y-%m-%d").map_err(|e| {
        actix_web::error::ErrorBadRequest(format!("Invalid start_date format: {}", e))
    })?;

    let end_date = NaiveDate::parse_from_str(&query.end_date, "%Y-%m-%d").map_err(|e| {
        actix_web::error::ErrorBadRequest(format!("Invalid end_date format: {}", e))
    })?;

    // Parse service filter
    let service_filter = query
        .service_filter
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

    let request = CostAnalysisRequest {
        account_id: query.account_id.clone(),
        start_date,
        end_date,
        service_filter,
        granularity: query
            .granularity
            .clone()
            .unwrap_or_else(|| "MONTHLY".to_string()),
    };

    match cost_service.fetch_cost_data(&request).await {
        Ok(metrics) => {
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "total_cost": metrics.total_cost,
                    "service_breakdown": metrics.service_breakdown,
                    "monthly_trend": metrics.monthly_trend,
                    "anomalies_detected": metrics.anomalies_detected.len(),
                    "anomalies": metrics.anomalies_detected
                }),
                message: "Cost data fetched successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to fetch cost data: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Get monthly cost aggregates
pub async fn get_monthly_aggregates(
    repository: web::Data<Arc<CostAnalyticsRepository>>,
    query: web::Query<MonthlyAggregatesQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!(
        "Getting monthly aggregates for account {}",
        query.account_id
    );

    match repository
        .get_monthly_aggregates_by_account(&query.account_id, query.months)
        .await
    {
        Ok(aggregates) => {
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "aggregates": aggregates.into_iter().map(|a| serde_json::json!({
                        "id": a.id,
                        "service_name": a.service_name,
                        "month_year": a.month_year,
                        "total_cost": a.total_cost,
                        "cost_change_pct": a.cost_change_pct,
                        "cost_change_amount": a.cost_change_amount,
                        "is_anomaly": a.is_anomaly,
                        "anomaly_score": a.anomaly_score
                    })).collect::<Vec<_>>()
                }),
                message: "Monthly aggregates retrieved successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get monthly aggregates: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Get top services by cost for a specific month
pub async fn get_top_services(
    repository: web::Data<Arc<CostAnalyticsRepository>>,
    query: web::Query<TopServicesQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!(
        "Getting top services for account {} and month {}",
        query.account_id,
        query.month_year
    );

    // Parse month_year
    let month_date = NaiveDate::parse_from_str(&format!("{}-01", query.month_year), "%Y-%m-%d")
        .map_err(|e| {
            actix_web::error::ErrorBadRequest(format!("Invalid month_year format: {}", e))
        })?;

    match repository
        .get_top_services_by_cost(&query.account_id, month_date, query.limit)
        .await
    {
        Ok(services) => {
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "top_services": services.into_iter().map(|s| serde_json::json!({
                        "service_name": s.service_name,
                        "total_cost": s.total_cost,
                        "cost_change_pct": s.cost_change_pct,
                        "cost_change_amount": s.cost_change_amount,
                        "is_anomaly": s.is_anomaly,
                        "anomaly_score": s.anomaly_score
                    })).collect::<Vec<_>>()
                }),
                message: "Top services retrieved successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get top services: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Get cost anomalies
pub async fn get_cost_anomalies(
    repository: web::Data<Arc<CostAnalyticsRepository>>,
    query: web::Query<AnomaliesQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!("Getting cost anomalies for account {}", query.account_id);

    match repository
        .get_cost_anomalies_by_account(
            &query.account_id,
            query.severity.clone(),
            query.status.clone(),
        )
        .await
    {
        Ok(anomalies) => {
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "anomalies": anomalies.into_iter().map(|a| serde_json::json!({
                        "id": a.id,
                        "service_name": a.service_name,
                        "anomaly_type": a.anomaly_type,
                        "severity": a.severity,
                        "detected_date": a.detected_date,
                        "anomaly_score": a.anomaly_score,
                        "baseline_cost": a.baseline_cost,
                        "actual_cost": a.actual_cost,
                        "cost_difference": a.cost_difference,
                        "percentage_change": a.percentage_change,
                        "description": a.description,
                        "status": a.status
                    })).collect::<Vec<_>>()
                }),
                message: "Cost anomalies retrieved successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get cost anomalies: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Get LLM-generated cost insights
pub async fn get_cost_insights(
    repository: web::Data<Arc<CostAnalyticsRepository>>,
    query: web::Query<InsightsQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!("Getting cost insights for account {}", query.account_id);

    match repository
        .get_recent_insights(&query.account_id, query.insight_type.clone(), query.limit)
        .await
    {
        Ok(insights) => {
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "insights": insights.into_iter().map(|i| serde_json::json!({
                        "id": i.id,
                        "insight_type": i.insight_type,
                        "llm_provider": i.llm_provider,
                        "llm_model": i.llm_model,
                        "summary": i.summary,
                        "recommendations": i.recommendations,
                        "confidence_score": i.confidence_score,
                        "created_at": i.created_at
                    })).collect::<Vec<_>>()
                }),
                message: "Cost insights retrieved successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get cost insights: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Trigger manual computation of monthly aggregates
pub async fn compute_monthly_aggregates(
    cost_service: web::Data<Arc<AwsCostAnalyticsService>>,
    query: web::Query<MonthlyAggregatesQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!(
        "Computing monthly aggregates for account {}",
        query.account_id
    );

    match cost_service
        .compute_monthly_aggregates(&query.account_id)
        .await
    {
        Ok(()) => {
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({}),
                message: "Monthly aggregates computed successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to compute monthly aggregates: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Get cost summary dashboard data
pub async fn get_cost_summary(
    cost_service: web::Data<Arc<AwsCostAnalyticsService>>,
    query: web::Query<MonthlyAggregatesQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!("Getting cost summary for account {}", query.account_id);

    match cost_service.get_cost_summary(&query.account_id).await {
        Ok(summary) => {
            let response = CostAnalysisResponse {
                success: true,
                data: summary,
                message: "Cost summary retrieved successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get cost summary: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Get costs by specific resources
pub async fn get_resource_costs(
    repository: web::Data<Arc<CostAnalyticsRepository>>,
    query: web::Query<ResourceCostQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!("Getting resource costs for account {}", query.account_id);

    // Parse dates
    let start_date = NaiveDate::parse_from_str(&query.start_date, "%Y-%m-%d").map_err(|e| {
        actix_web::error::ErrorBadRequest(format!("Invalid start_date format: {}", e))
    })?;

    let end_date = NaiveDate::parse_from_str(&query.end_date, "%Y-%m-%d").map_err(|e| {
        actix_web::error::ErrorBadRequest(format!("Invalid end_date format: {}", e))
    })?;

    match repository
        .get_resource_costs(
            &query.account_id,
            start_date,
            end_date,
            query.resource_id.as_deref(),
            query.service_name.as_deref(),
            query.region.as_deref(),
            query.availability_zone.as_deref(),
            query.instance_type.as_deref(),
            query.limit,
        )
        .await
    {
        Ok(resources) => {
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "resources": resources.into_iter().map(|r| serde_json::json!({
                        "id": r.id,
                        "account_id": r.account_id,
                        "service_name": r.service_name,
                        "usage_type": r.usage_type,
                        "operation": r.operation,
                        "region": r.region,
                        "resource_id": r.tags.as_ref().and_then(|t| t.get("resource_id")),
                        "usage_start": r.usage_start,
                        "usage_end": r.usage_end,
                        "unblended_cost": r.unblended_cost,
                        "blended_cost": r.blended_cost,
                        "usage_amount": r.usage_amount,
                        "usage_unit": r.usage_unit,
                        "currency": r.currency
                    })).collect::<Vec<_>>()
                }),
                message: "Resource costs retrieved successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get resource costs: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Generate LLM analysis for a specific cost trend or anomaly
pub async fn analyze_cost_with_llm(
    cost_service: web::Data<Arc<AwsCostAnalyticsService>>,
    path: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    let anomaly_id = path.into_inner();
    tracing::info!("Generating LLM analysis for anomaly {}", anomaly_id);

    // This would typically fetch the anomaly and generate insights
    // For now, return a placeholder response
    let response = CostAnalysisResponse {
        success: true,
        data: serde_json::json!({
            "message": "LLM analysis functionality would be implemented here",
            "anomaly_id": anomaly_id
        }),
        message: "LLM analysis feature coming soon".to_string(),
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Get newly added resources across accounts
pub async fn get_new_resources(
    aws_resource_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    query: web::Query<NewResourcesQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!("Getting newly added resources");

    let days_back = query.days_back.unwrap_or(7); // Default to last 7 days
    let limit = query.limit.unwrap_or(100); // Default limit

    match aws_resource_repo
        .find_newly_added_resources(query.account_id.clone(), days_back, Some(limit))
        .await
    {
        Ok(resources) => {
            let total_count = resources.len();
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "new_resources": resources.into_iter().map(|r| serde_json::json!({
                        "id": r.id,
                        "account_id": r.account_id,
                        "resource_type": r.resource_type,
                        "resource_id": r.resource_id,
                        "arn": r.arn,
                        "name": r.name,
                        "region": r.region,
                        "created_at": r.created_at,
                        "tags": r.tags
                    })).collect::<Vec<_>>(),
                    "days_back": days_back,
                    "total_count": total_count
                }),
                message: format!("Found {} newly added resources in the last {} days", total_count, days_back),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get new resources: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

/// Get resource count changes over time
pub async fn get_resource_count_trends(
    aws_resource_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    query: web::Query<NewResourcesQuery>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    tracing::info!("Getting resource count trends");

    let days_back = query.days_back.unwrap_or(30); // Default to last 30 days

    match aws_resource_repo
        .get_resource_count_changes(query.account_id.clone(), days_back)
        .await
    {
        Ok(trends) => {
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "trends": trends.into_iter().map(|(date, count)| serde_json::json!({
                        "date": date,
                        "new_resources_count": count
                    })).collect::<Vec<_>>(),
                    "days_back": days_back
                }),
                message: "Resource count trends retrieved successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get resource count trends: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

#[tracing::instrument(skip(aws_resource_repo))]
pub async fn detect_cost_increases(
    query: web::Query<CostIncreaseQuery>,
    aws_resource_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    _claims: web::ReqData<Claims>,
) -> ActixResult<HttpResponse> {
    let account_id = query.account_id.clone();
    let days_back = query.days_back.unwrap_or(30);
    let threshold_percentage = query.threshold_percentage.unwrap_or(20.0);
    let min_cost_threshold = query.min_cost_threshold.unwrap_or(10.0);

    match aws_resource_repo
        .detect_cost_increases(account_id, days_back, threshold_percentage, min_cost_threshold)
        .await
    {
        Ok(cost_increases) => {
            let total_count = cost_increases.len();
            let response = CostAnalysisResponse {
                success: true,
                data: serde_json::json!({
                    "cost_increases": cost_increases,
                    "days_back": days_back,
                    "threshold_percentage": threshold_percentage,
                    "min_cost_threshold": min_cost_threshold,
                    "total_count": total_count
                }),
                message: format!(
                    "Found {} resources with cost increases of {}% or more in the last {} days",
                    total_count, threshold_percentage, days_back
                ),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to detect cost increases: {}", e);
            let error_response = ErrorResponse::from(e);
            Ok(HttpResponse::InternalServerError().json(error_response))
        }
    }
}

#[derive(Deserialize)]
pub struct ResourceCostHistoryQuery {
    pub resource_id: String,
    pub account_id: Option<String>,
    pub days_back: Option<i64>,
    pub granularity: Option<String>, // "daily" or "weekly"
}

pub async fn get_resource_cost_history(
    query: web::Query<ResourceCostHistoryQuery>,
    claims: web::ReqData<Claims>,
    aws_resource_repo: web::Data<AwsResourceRepository>,
) -> Result<HttpResponse, AppError> {
    let days_back = query.days_back.unwrap_or(30);
    let granularity = query.granularity.clone().unwrap_or_else(|| "daily".to_string());

    if days_back > 365 {
        return Err(AppError::Validation("days_back cannot exceed 365 days".to_string()));
    }

    if !["daily", "weekly"].contains(&granularity.as_str()) {
        return Err(AppError::Validation("granularity must be 'daily' or 'weekly'".to_string()));
    }

    let granularity_clone = granularity.clone();
    let cost_history = aws_resource_repo
        .get_resource_cost_history(
            &query.resource_id,
            query.account_id.clone(),
            days_back,
            granularity,
        )
        .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_id": query.resource_id,
        "account_id": query.account_id,
        "days_back": days_back,
        "granularity": granularity_clone,
        "data": cost_history,
        "total_periods": cost_history.len()
    })))
}
