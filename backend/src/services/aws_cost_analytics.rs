use aws_config::SdkConfig;
use aws_sdk_costexplorer::{
    operation::get_cost_and_usage::GetCostAndUsageInput, types::*, Client as CostExplorerClient,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use sea_orm::prelude::Decimal;
use sea_orm::ActiveValue;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{
    aws_cost_anomalies::ActiveModel as CostAnomalyActiveModel,
    aws_cost_data::ActiveModel as CostDataActiveModel,
    aws_cost_insights::ActiveModel as CostInsightActiveModel,
    aws_monthly_cost_aggregates::ActiveModel as MonthlyCostAggregateActiveModel,
};
use crate::repositories::cost_analytics::CostAnalyticsRepository;
use crate::repositories::aws_account::AwsAccountRepository;
use crate::services::aws::AwsService;
use crate::services::llm::LlmIntegrationService;

#[derive(Debug, Clone)]
pub struct CostMetrics {
    pub total_cost: f64,
    pub service_breakdown: HashMap<String, f64>,
    pub monthly_trend: Vec<(String, f64)>,
    pub anomalies_detected: Vec<CostAnomaly>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostAnomaly {
    pub service_name: String,
    pub anomaly_type: String,
    pub severity: String,
    pub baseline_cost: f64,
    pub actual_cost: f64,
    pub percentage_change: f64,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct CostAnalysisRequest {
    pub account_id: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub service_filter: Option<Vec<String>>,
    pub granularity: String, // "DAILY", "MONTHLY"
}

#[derive(Debug)]
pub struct AwsCostAnalyticsService {
    repository: Arc<CostAnalyticsRepository>,
    aws_account_repo: Arc<AwsAccountRepository>,
    aws_service: Arc<AwsService>,
    llm_service: Arc<LlmIntegrationService>,
}

impl AwsCostAnalyticsService {
    pub fn new(
        repository: Arc<CostAnalyticsRepository>,
        aws_account_repo: Arc<AwsAccountRepository>,
        aws_service: Arc<AwsService>,
        llm_service: Arc<LlmIntegrationService>,
    ) -> Self {
        Self {
            repository,
            aws_account_repo,
            aws_service,
            llm_service,
        }
    }

    /// Fetch real-time cost data from AWS Cost Explorer API
    pub async fn fetch_cost_data(
        &self,
        request: &CostAnalysisRequest,
    ) -> Result<CostMetrics, AppError> {
        tracing::info!(
            "Fetching cost data for account {} from {} to {}",
            request.account_id,
            request.start_date,
            request.end_date
        );

        // Get AWS account details
        let aws_account = self
            .aws_account_repo
            .get_by_account_id(&request.account_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("AWS account {} not found", request.account_id))
            })?;

        // Convert to DTO for AWS service
        let aws_account_dto = crate::models::aws_account::AwsAccountDto::from(aws_account);

        // Create AWS SDK config for this account
        let aws_config = self.aws_service.get_aws_sdk_config(&aws_account_dto).await?;
        let cost_explorer_client = CostExplorerClient::new(&aws_config);

        // Create time period
        let time_period = DateInterval::builder()
            .start(request.start_date.format("%Y-%m-%d").to_string())
            .end(request.end_date.format("%Y-%m-%d").to_string())
            .build()
            .map_err(|e| AppError::CloudProvider(format!("Failed to build time period: {}", e)))?;

        // Create granularity
        let granularity = Granularity::from(request.granularity.as_str());

        // Execute the API call
        let response = cost_explorer_client
            .get_cost_and_usage()
            .time_period(time_period)
            .granularity(granularity)
            .set_metrics(Some(vec![
                "UnblendedCost".to_string(),
                "BlendedCost".to_string(),
                "UsageQuantity".to_string(),
            ]))
            .set_group_by(Some(vec![
                GroupDefinition::builder()
                    .r#type(GroupDefinitionType::Dimension)
                    .key("SERVICE")
                    .build(),
                GroupDefinition::builder()
                    .r#type(GroupDefinitionType::Dimension)
                    .key("AZ")  // Availability Zone
                    .build(),
                GroupDefinition::builder()
                    .r#type(GroupDefinitionType::Dimension)
                    .key("INSTANCE_TYPE")  // For EC2 instances
                    .build(),
                GroupDefinition::builder()
                    .r#type(GroupDefinitionType::Dimension)
                    .key("RESOURCE_ID")  // Individual resource identifier
                    .build(),
                GroupDefinition::builder()
                    .r#type(GroupDefinitionType::Dimension)
                    .key("USAGE_TYPE")
                    .build(),
            ]))
            .send()
            .await
            .map_err(|e| AppError::CloudProvider(format!("Cost Explorer API error: {}", e)))?;

        // Process the response
        let mut cost_data_models = Vec::new();
        let mut service_breakdown = HashMap::new();
        let mut total_cost = 0.0;

        if let Some(results_by_time) = response.results_by_time {
            for time_result in results_by_time {
                let time_period = time_result.time_period.ok_or_else(|| {
                    AppError::Validation("Missing time period in response".to_string())
                })?;
                let start_date = NaiveDate::parse_from_str(&time_period.start, "%Y-%m-%d")
                    .map_err(|e| AppError::Validation(format!("Invalid date format: {}", e)))?;
                let end_date = NaiveDate::parse_from_str(&time_period.end, "%Y-%m-%d")
                    .map_err(|e| AppError::Validation(format!("Invalid date format: {}", e)))?;

                if let Some(groups) = time_result.groups {
                    for group in groups {
                        let keys = group.keys.unwrap_or_default();
                        
                        // Extract dimensions: SERVICE, AZ, INSTANCE_TYPE, RESOURCE_ID, USAGE_TYPE
                        let service_name = keys.get(0).cloned().unwrap_or_default();
                        let availability_zone = keys.get(1).cloned();
                        let instance_type = keys.get(2).cloned();
                        let resource_id = keys.get(3).cloned();
                        let usage_type = keys.get(4).cloned();

                        if let Some(metrics) = group.metrics {
                            let unblended_cost = metrics
                                .get("UnblendedCost")
                                .and_then(|m| m.amount.as_ref())
                                .and_then(|a| a.parse::<f64>().ok())
                                .unwrap_or(0.0);

                            let blended_cost = metrics
                                .get("BlendedCost")
                                .and_then(|m| m.amount.as_ref())
                                .and_then(|a| a.parse::<f64>().ok())
                                .unwrap_or(0.0);

                            let usage_amount = metrics
                                .get("UsageQuantity")
                                .and_then(|m| m.amount.as_ref())
                                .and_then(|a| a.parse::<f64>().ok());

                            let usage_unit =
                                metrics.get("UsageQuantity").and_then(|m| m.unit.clone());

                            // Update service breakdown
                            *service_breakdown.entry(service_name.clone()).or_insert(0.0) +=
                                unblended_cost;
                            total_cost += unblended_cost;

                            // Create cost data model for database storage
                            let cost_data = CostDataActiveModel {
                                id: ActiveValue::Set(Uuid::new_v4()),
                                account_id: ActiveValue::Set(request.account_id.clone()),
                                service_name: ActiveValue::Set(service_name),
                                usage_type: ActiveValue::Set(usage_type),
                                operation: ActiveValue::Set(instance_type), // Store instance type in operation field
                                region: ActiveValue::Set(availability_zone), // Store AZ in region field
                                usage_start: ActiveValue::Set(start_date),
                                usage_end: ActiveValue::Set(end_date),
                                unblended_cost: ActiveValue::Set(
                                    Decimal::from_f64_retain(unblended_cost).unwrap_or_default(),
                                ),
                                blended_cost: ActiveValue::Set(
                                    Decimal::from_f64_retain(blended_cost).unwrap_or_default(),
                                ),
                                usage_amount: ActiveValue::Set(
                                    usage_amount
                                        .map(|u| Decimal::from_f64_retain(u).unwrap_or_default()),
                                ),
                                usage_unit: ActiveValue::Set(usage_unit),
                                currency: ActiveValue::Set("USD".to_string()),
                                tags: ActiveValue::Set(resource_id.map(|rid| {
                                    serde_json::json!({ "resource_id": rid })
                                })), // Store resource_id in tags
                                created_at: ActiveValue::Set(Utc::now().into()),
                                updated_at: ActiveValue::Set(Utc::now().into()),
                            };

                            cost_data_models.push(cost_data);
                        }
                    }
                }
            }
        }

        // Store cost data in database
        if !cost_data_models.is_empty() {
            self.repository.insert_cost_data(cost_data_models).await?;
        }

        // Detect anomalies
        let anomalies = self
            .detect_cost_anomalies(&request.account_id, &service_breakdown)
            .await?;

        // Generate monthly trend
        let monthly_trend = self
            .repository
            .calculate_monthly_totals(&request.account_id)
            .await?;

        Ok(CostMetrics {
            total_cost,
            service_breakdown,
            monthly_trend,
            anomalies_detected: anomalies,
        })
    }

    /// Compute monthly aggregates and detect anomalies
    pub async fn compute_monthly_aggregates(&self, account_id: &str) -> Result<(), AppError> {
        tracing::info!("Computing monthly aggregates for account {}", account_id);

        // Get the current month's start date
        let now = Utc::now().naive_utc().date();
        let current_month = NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
            .ok_or_else(|| AppError::Validation("Invalid date".to_string()))?;

        // Get the previous month for comparison
        let previous_month = if current_month.month() == 1 {
            NaiveDate::from_ymd_opt(current_month.year() - 1, 12, 1)
        } else {
            NaiveDate::from_ymd_opt(current_month.year(), current_month.month() - 1, 1)
        }
        .ok_or_else(|| AppError::Validation("Invalid previous month date".to_string()))?;

        // Get cost data for the current month
        let month_start = current_month;
        let month_end = if current_month.month() == 12 {
            NaiveDate::from_ymd_opt(current_month.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(current_month.year(), current_month.month() + 1, 1)
        }
        .ok_or_else(|| AppError::Validation("Invalid month end date".to_string()))?;

        let current_month_data = self
            .repository
            .get_cost_data_by_date_range(account_id, month_start, month_end, None)
            .await?;

        // Group by service and calculate totals
        let mut service_totals: HashMap<String, (f64, f64, String)> = HashMap::new();

        for cost_data in current_month_data {
            let service = cost_data.service_name.clone();
            let cost = cost_data
                .unblended_cost
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0);
            let usage = cost_data
                .usage_amount
                .map(|u| u.to_string().parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);
            let unit = cost_data.usage_unit.unwrap_or_default();

            let entry = service_totals.entry(service).or_insert((0.0, 0.0, unit));
            entry.0 += cost;
            entry.1 += usage;
        }

        // Get previous month's aggregates for comparison
        let previous_aggregates = self
            .repository
            .get_monthly_aggregates_by_account(account_id, Some(1))
            .await?;

        let mut previous_service_costs: HashMap<String, f64> = HashMap::new();
        let previous_month_str = previous_month.format("%Y-%m").to_string();
        for agg in previous_aggregates {
            if agg.month_year.format("%Y-%m").to_string() == previous_month_str {
                let cost = agg.total_cost.to_string().parse::<f64>().unwrap_or(0.0);
                previous_service_costs.insert(agg.service_name, cost);
            }
        }

        // Create monthly aggregates
        for (service_name, (total_cost, usage_amount, usage_unit)) in service_totals {
            let previous_cost = previous_service_costs
                .get(&service_name)
                .copied()
                .unwrap_or(0.0);
            let cost_change_amount = total_cost - previous_cost;
            let cost_change_pct = if previous_cost > 0.0 {
                (cost_change_amount / previous_cost) * 100.0
            } else {
                0.0
            };

            // Simple anomaly detection based on percentage change
            let anomaly_score = (cost_change_pct.abs() / 100.0).min(10.0);
            let is_anomaly = cost_change_pct.abs() > 50.0 && total_cost > 10.0; // More than 50% change and significant cost

            let aggregate = MonthlyCostAggregateActiveModel {
                id: ActiveValue::Set(Uuid::new_v4()),
                account_id: ActiveValue::Set(account_id.to_string()),
                service_name: ActiveValue::Set(service_name.clone()),
                month_year: ActiveValue::Set(current_month),
                total_cost: ActiveValue::Set(
                    Decimal::from_f64_retain(total_cost).unwrap_or_default(),
                ),
                usage_amount: ActiveValue::Set(Some(
                    Decimal::from_f64_retain(usage_amount).unwrap_or_default(),
                )),
                usage_unit: ActiveValue::Set(Some(usage_unit)),
                cost_change_pct: ActiveValue::Set(Some(
                    Decimal::from_f64_retain(cost_change_pct).unwrap_or_default(),
                )),
                cost_change_amount: ActiveValue::Set(Some(
                    Decimal::from_f64_retain(cost_change_amount).unwrap_or_default(),
                )),
                anomaly_score: ActiveValue::Set(Some(
                    Decimal::from_f64_retain(anomaly_score).unwrap_or_default(),
                )),
                is_anomaly: ActiveValue::Set(is_anomaly),
                tags_summary: ActiveValue::NotSet,
                created_at: ActiveValue::Set(Utc::now().into()),
                updated_at: ActiveValue::Set(Utc::now().into()),
            };

            let saved_aggregate = self.repository.insert_monthly_aggregate(aggregate).await?;

            // If anomaly detected, create anomaly record and generate LLM insight
            if is_anomaly {
                let anomaly_type = if cost_change_pct > 0.0 {
                    "spike"
                } else {
                    "drop"
                };
                let severity = if cost_change_pct.abs() > 100.0 {
                    "high"
                } else {
                    "medium"
                };

                let anomaly = CostAnomalyActiveModel {
                    id: ActiveValue::Set(Uuid::new_v4()),
                    account_id: ActiveValue::Set(account_id.to_string()),
                    service_name: ActiveValue::Set(service_name.clone()),
                    anomaly_type: ActiveValue::Set(anomaly_type.to_string()),
                    severity: ActiveValue::Set(severity.to_string()),
                    detected_date: ActiveValue::Set(current_month),
                    anomaly_score: ActiveValue::Set(
                        Decimal::from_f64_retain(anomaly_score).unwrap_or_default(),
                    ),
                    baseline_cost: ActiveValue::Set(Some(
                        Decimal::from_f64_retain(previous_cost).unwrap_or_default(),
                    )),
                    actual_cost: ActiveValue::Set(
                        Decimal::from_f64_retain(total_cost).unwrap_or_default(),
                    ),
                    cost_difference: ActiveValue::Set(Some(
                        Decimal::from_f64_retain(cost_change_amount).unwrap_or_default(),
                    )),
                    percentage_change: ActiveValue::Set(Some(
                        Decimal::from_f64_retain(cost_change_pct).unwrap_or_default(),
                    )),
                    description: ActiveValue::Set(Some(format!(
                        "{} cost {} by {:.1}% ({:.2} -> {:.2})",
                        service_name,
                        anomaly_type,
                        cost_change_pct.abs(),
                        previous_cost,
                        total_cost
                    ))),
                    status: ActiveValue::Set("open".to_string()),
                    created_at: ActiveValue::Set(Utc::now().into()),
                    updated_at: ActiveValue::Set(Utc::now().into()),
                };

                let saved_anomaly = self.repository.insert_cost_anomaly(anomaly).await?;

                // Generate LLM insight for the anomaly
                self.generate_anomaly_insight(&saved_anomaly, &saved_aggregate)
                    .await?;
            }
        }

        Ok(())
    }

    /// Detect cost anomalies using statistical methods
    async fn detect_cost_anomalies(
        &self,
        account_id: &str,
        service_costs: &HashMap<String, f64>,
    ) -> Result<Vec<CostAnomaly>, AppError> {
        let mut anomalies = Vec::new();

        // Get historical data for comparison
        let historical_aggregates = self
            .repository
            .get_monthly_aggregates_by_account(account_id, Some(6))
            .await?;

        let mut service_history: HashMap<String, Vec<f64>> = HashMap::new();
        for agg in historical_aggregates {
            let cost_value = agg.total_cost.to_string().parse::<f64>().unwrap_or(0.0);
            service_history
                .entry(agg.service_name)
                .or_insert_with(Vec::new)
                .push(cost_value);
        }

        // Analyze each service for anomalies
        for (service_name, &current_cost) in service_costs {
            if let Some(history) = service_history.get(service_name) {
                if history.len() >= 3 {
                    // Need at least 3 data points
                    let mean: f64 = history.iter().sum::<f64>() / history.len() as f64;
                    let variance: f64 = history.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                        / history.len() as f64;
                    let std_dev = variance.sqrt();

                    // Z-score calculation
                    let z_score = if std_dev > 0.0 {
                        (current_cost - mean).abs() / std_dev
                    } else {
                        0.0
                    };

                    // Consider it an anomaly if z-score > 2 (roughly 95% confidence)
                    if z_score > 2.0 && current_cost > 5.0 {
                        // Significant cost threshold
                        let percentage_change = if mean > 0.0 {
                            ((current_cost - mean) / mean) * 100.0
                        } else {
                            0.0
                        };
                        let severity = if z_score > 3.0 { "high" } else { "medium" };
                        let anomaly_type = if current_cost > mean { "spike" } else { "drop" };

                        anomalies.push(CostAnomaly {
                            service_name: service_name.clone(),
                            anomaly_type: anomaly_type.to_string(),
                            severity: severity.to_string(),
                            baseline_cost: mean,
                            actual_cost: current_cost,
                            percentage_change,
                            description: format!(
                                "{} cost {} detected: {:.2} vs baseline {:.2} ({:.1}% change, z-score: {:.2})",
                                service_name, anomaly_type, current_cost, mean, percentage_change, z_score
                            ),
                        });
                    }
                }
            }
        }

        Ok(anomalies)
    }

    /// Generate LLM-powered insights for cost anomalies
    async fn generate_anomaly_insight(
        &self,
        anomaly: &crate::models::aws_cost_anomalies::Model,
        aggregate: &crate::models::aws_monthly_cost_aggregates::Model,
    ) -> Result<(), AppError> {
        // Create a prompt for the LLM to analyze the cost anomaly
        let prompt = format!(
            r#"Analyze this AWS cost anomaly and provide insights:

Service: {}
Anomaly Type: {}
Severity: {}
Baseline Cost: ${:.2}
Actual Cost: ${:.2}
Percentage Change: {:.1}%
Detection Date: {}

Please provide:
1. Likely root causes for this cost change
2. Immediate investigation steps
3. Recommended actions to address the issue
4. Potential cost optimization opportunities

Respond in JSON format with the following structure:
{{
    "summary": "Brief explanation of the anomaly",
    "root_causes": ["cause1", "cause2", "cause3"],
    "investigation_steps": ["step1", "step2", "step3"],
    "recommendations": ["action1", "action2", "action3"],
    "urgency": "low|medium|high"
}}
"#,
            anomaly.service_name,
            anomaly.anomaly_type,
            anomaly.severity,
            anomaly
                .baseline_cost
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0),
            anomaly
                .actual_cost
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0),
            anomaly
                .percentage_change
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0),
            anomaly.detected_date
        );

        // Find the first available LLM provider
        // Note: We'll temporarily skip LLM insights and add a TODO
        tracing::warn!(
            "LLM insight generation not yet implemented - would analyze anomaly {} in service {}",
            anomaly.id,
            anomaly.service_name
        );

        // TODO: Implement LLM insight generation once LlmIntegrationService exposes a method to get providers
        // For now, we'll just log the anomaly and continue

        Ok(())
    }

    /// Get cost summary for dashboard
    pub async fn get_cost_summary(&self, account_id: &str) -> Result<serde_json::Value, AppError> {
        // Get recent aggregates
        let aggregates = self
            .repository
            .get_monthly_aggregates_by_account(account_id, Some(6))
            .await?;

        // Get current month totals
        let monthly_totals = self.repository.calculate_monthly_totals(account_id).await?;

        // Get recent anomalies
        let anomalies = self
            .repository
            .get_cost_anomalies_by_account(account_id, None, Some("open".to_string()))
            .await?;

        // Get top services for current month
        let now = Utc::now().naive_utc().date();
        let current_month = NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
            .ok_or_else(|| AppError::Validation("Invalid date".to_string()))?;
        let top_services = self
            .repository
            .get_top_services_by_cost(account_id, current_month, Some(10))
            .await?;

        Ok(serde_json::json!({
            "account_id": account_id,
            "monthly_totals": monthly_totals,
            "top_services": top_services.iter().map(|s| serde_json::json!({
                "service_name": s.service_name,
                "total_cost": s.total_cost,
                "cost_change_pct": s.cost_change_pct,
                "is_anomaly": s.is_anomaly
            })).collect::<Vec<_>>(),
            "anomalies": anomalies.iter().map(|a| serde_json::json!({
                "id": a.id,
                "service_name": a.service_name,
                "anomaly_type": a.anomaly_type,
                "severity": a.severity,
                "actual_cost": a.actual_cost,
                "percentage_change": a.percentage_change,
                "description": a.description
            })).collect::<Vec<_>>(),
            "summary": {
                "total_services": top_services.len(),
                "anomalies_count": anomalies.len(),
                "months_analyzed": monthly_totals.len()
            }
        }))
    }
}
