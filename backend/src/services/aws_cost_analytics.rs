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
use crate::repositories::aws_account::AwsAccountRepository;
use crate::repositories::cost_analytics::CostAnalyticsRepository;
use crate::services::aws::AwsService;
use crate::services::llm::LlmIntegrationService;
use crate::repositories::llm_provider::LlmProviderRepository;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_point_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trend_slope: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rolling_mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rolling_std_dev: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month_over_month_change: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month_over_month_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_points_analyzed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasonality_ratio: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_std_dev: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_methods: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct CostAnalysisRequest {
    pub account_id: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub service_filter: Option<Vec<String>>,
    pub granularity: String, // "DAILY", "MONTHLY"
}

#[derive(Debug, Clone)]
struct AdvancedAnomalyMetrics {
    is_anomaly: bool,
    severity: String,
    anomaly_type: String,
    baseline_cost: f64,
    baseline_mean: f64,
    baseline_std_dev: f64,
    rolling_mean: f64,
    rolling_std_dev: f64,
    percent_change: f64,
    month_over_month_change: f64,
    z_score: f64,
    change_point_score: f64,
    trend_slope: f64,
    confidence: f64,
    data_points: usize,
    seasonality_ratio: Option<f64>,
    composite_score: f64,
    detection_methods: Vec<String>,
}

impl Default for AdvancedAnomalyMetrics {
    fn default() -> Self {
        Self {
            is_anomaly: false,
            severity: "low".to_string(),
            anomaly_type: "stable".to_string(),
            baseline_cost: 0.0,
            baseline_mean: 0.0,
            baseline_std_dev: 0.0,
            rolling_mean: 0.0,
            rolling_std_dev: 0.0,
            percent_change: 0.0,
            month_over_month_change: 0.0,
            z_score: 0.0,
            change_point_score: 0.0,
            trend_slope: 0.0,
            confidence: 0.0,
            data_points: 0,
            seasonality_ratio: None,
            composite_score: 0.0,
            detection_methods: Vec::new(),
        }
    }
}

impl AdvancedAnomalyMetrics {
    fn from_history(history: &[(NaiveDate, f64)], current_cost: f64) -> Self {
        let mut metrics = AdvancedAnomalyMetrics::default();
        metrics.data_points = history.len();

        if history.is_empty() {
            metrics.baseline_cost = 0.0;
            metrics.baseline_mean = 0.0;
            metrics.month_over_month_change = current_cost;
            metrics.percent_change = 0.0;

            if current_cost > 100.0 {
                metrics.is_anomaly = true;
                metrics.severity = if current_cost > 500.0 {
                    "high".to_string()
                } else {
                    "medium".to_string()
                };
                metrics.anomaly_type = "new-cost-center".to_string();
                metrics.composite_score = (current_cost / 1000.0).min(1.0) * 10.0;
                metrics.confidence = 0.6;
                metrics.detection_methods.push("new-service".to_string());
            }

            return metrics;
        }

        let mut sorted_history = history.to_vec();
        sorted_history.sort_by_key(|(date, _)| *date);
        let history_values: Vec<f64> = sorted_history.iter().map(|(_, cost)| *cost).collect();

        metrics.baseline_cost = *history_values.last().unwrap_or(&0.0);
        metrics.baseline_mean = Self::mean(&history_values);
        metrics.baseline_std_dev = Self::population_std_dev(&history_values);

        let (rolling_mean, rolling_std_dev) = Self::rolling_stats(&history_values, 3);
        metrics.rolling_mean = rolling_mean;
        metrics.rolling_std_dev = rolling_std_dev;

        metrics.month_over_month_change = current_cost - metrics.baseline_cost;
        let denom = if metrics.baseline_cost.abs() < 1.0 {
            1.0
        } else {
            metrics.baseline_cost
        };
        metrics.percent_change = ((current_cost - metrics.baseline_cost) / denom) * 100.0;

        metrics.z_score = if metrics.baseline_std_dev > 0.0 {
            (current_cost - metrics.baseline_mean) / metrics.baseline_std_dev
        } else {
            0.0
        };

        metrics.change_point_score = if metrics.rolling_std_dev > 0.0 {
            (current_cost - metrics.rolling_mean).abs() / metrics.rolling_std_dev
        } else {
            (current_cost - metrics.rolling_mean).abs() / metrics.rolling_mean.abs().max(1.0)
        };

        metrics.trend_slope = Self::compute_slope(&history_values);
        metrics.seasonality_ratio = Self::seasonality_ratio(&sorted_history, current_cost);

        let mut detection_methods = Vec::new();

        if metrics.z_score.abs() >= 2.25 {
            detection_methods.push("z-score".to_string());
        }

        if metrics.percent_change.abs() >= 35.0 {
            detection_methods.push("percent-change".to_string());
        }

        if metrics.change_point_score >= 2.0 {
            detection_methods.push("rolling-breakout".to_string());
        }

        let slope_component = if metrics.rolling_mean.abs() > 0.0 {
            ((metrics.trend_slope * history_values.len() as f64) / metrics.rolling_mean).abs()
        } else {
            metrics.trend_slope.abs()
        };

        if slope_component >= 0.5 {
            detection_methods.push("trend-shift".to_string());
        }

        if metrics.baseline_mean > 50.0 && current_cost <= metrics.baseline_mean * 0.4 {
            detection_methods.push("cost-drop".to_string());
            if metrics.month_over_month_change == 0.0 {
                metrics.month_over_month_change = current_cost - metrics.baseline_mean;
            }
        }

        if metrics.baseline_cost < 5.0 && current_cost > 100.0 {
            detection_methods.push("new-service".to_string());
        }

        let z_component = (metrics.z_score.abs() / 3.0).min(1.0);
        let pct_component = (metrics.percent_change.abs() / 80.0).min(1.0);
        let change_component = (metrics.change_point_score / 3.0).min(1.0);
        let slope_component_norm = (slope_component / 1.5).min(1.0);

        let mut composite = (0.4 * z_component)
            + (0.3 * pct_component)
            + (0.2 * change_component)
            + (0.1 * slope_component_norm);

        if current_cost < 20.0 && metrics.baseline_mean < 20.0 {
            composite *= 0.6;
        }

        metrics.composite_score = composite.clamp(0.0, 1.0) * 10.0;

        let methods_hit = detection_methods.len();
        let anomaly_triggered = metrics.composite_score >= 5.0 || methods_hit >= 2;

        if anomaly_triggered {
            metrics.is_anomaly = true;
        }

        metrics.anomaly_type = if current_cost >= metrics.rolling_mean {
            "spike".to_string()
        } else {
            "drop".to_string()
        };

        metrics.severity = if metrics.composite_score >= 8.0 {
            "high".to_string()
        } else if metrics.composite_score >= 5.5 {
            "medium".to_string()
        } else if metrics.composite_score >= 4.0 {
            "guarded".to_string()
        } else {
            "low".to_string()
        };

        if metrics.severity == "low" && methods_hit >= 3 {
            metrics.severity = "medium".to_string();
            metrics.is_anomaly = true;
        }

        if metrics.baseline_cost < 5.0 && current_cost > 100.0 {
            metrics.is_anomaly = true;
            metrics.severity = "high".to_string();
            metrics.anomaly_type = "spike".to_string();
            metrics.composite_score = metrics.composite_score.max(7.0);
        }

        metrics.detection_methods = detection_methods;

        let method_component = (methods_hit as f64 / 4.0).min(1.0);
        let data_component = (metrics.data_points as f64 / 6.0).min(1.0);
        metrics.confidence = ((method_component * 0.6) + (data_component * 0.4))
            * composite.clamp(0.25, 1.0);
        metrics.confidence = metrics.confidence.clamp(0.0, 1.0);

        metrics
    }

    fn mean(values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        values.iter().sum::<f64>() / values.len() as f64
    }

    fn population_std_dev(values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        let mean = Self::mean(values);
        let variance = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;

        variance.sqrt()
    }

    fn rolling_stats(values: &[f64], window: usize) -> (f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0);
        }

        let window_size = values.len().min(window);
        let window_slice = &values[values.len() - window_size..];
        let mean = Self::mean(window_slice);
        let variance = if window_slice.len() < 2 {
            0.0
        } else {
            window_slice
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / window_slice.len() as f64
        };

        (mean, variance.sqrt())
    }

    fn compute_slope(values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        let n = values.len() as f64;
        let x_mean = ((0..values.len()).map(|i| i as f64).sum::<f64>()) / n;
        let y_mean = Self::mean(values);

        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (idx, value) in values.iter().enumerate() {
            let x = idx as f64;
            numerator += (x - x_mean) * (value - y_mean);
            denominator += (x - x_mean).powi(2);
        }

        if denominator == 0.0 {
            0.0
        } else {
            numerator / denominator
        }
    }

    fn seasonality_ratio(history: &[(NaiveDate, f64)], current_cost: f64) -> Option<f64> {
        if history.len() < 12 {
            return None;
        }

        let &(latest_date, _) = history.last()?;
        let target_year = latest_date.year() - 1;
        let target_month = latest_date.month();

        history
            .iter()
            .find(|(date, _)| date.year() == target_year && date.month() == target_month)
            .and_then(|(_, cost)| {
                if *cost > 0.0 {
                    Some(current_cost / cost.max(1.0))
                } else {
                    None
                }
            })
    }
}

#[derive(Debug)]
pub struct AwsCostAnalyticsService {
    repository: Arc<CostAnalyticsRepository>,
    aws_account_repo: Arc<AwsAccountRepository>,
    aws_service: Arc<AwsService>,
    llm_service: Arc<LlmIntegrationService>,
    llm_provider_repo: Arc<LlmProviderRepository>,
}

impl AwsCostAnalyticsService {
    pub fn new(
        repository: Arc<CostAnalyticsRepository>,
        aws_account_repo: Arc<AwsAccountRepository>,
        aws_service: Arc<AwsService>,
        llm_service: Arc<LlmIntegrationService>,
        llm_provider_repo: Arc<LlmProviderRepository>,
    ) -> Self {
        Self {
            repository,
            aws_account_repo,
            aws_service,
            llm_service,
            llm_provider_repo,
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

        let mut service_history: HashMap<String, Vec<(NaiveDate, f64)>> = HashMap::new();
        for agg in historical_aggregates {
            let cost_value = agg.total_cost.to_string().parse::<f64>().unwrap_or(0.0);
            service_history
                .entry(agg.service_name.clone())
                .or_insert_with(Vec::new)
                .push((agg.month_year, cost_value));
        }

        for history in service_history.values_mut() {
            history.sort_by_key(|(date, _)| *date);
        }

        // Analyze each service for anomalies
        for (service_name, &current_cost) in service_costs {
            let history = service_history
                .get(service_name)
                .cloned()
                .unwrap_or_default();

            let metrics = self.evaluate_advanced_anomaly(&history, current_cost);

            if !metrics.is_anomaly {
                continue;
            }

            let detection_methods = if metrics.detection_methods.is_empty() {
                None
            } else {
                Some(metrics.detection_methods.clone())
            };

            let description = format!(
                "{} {} detected: ${:.2} vs baseline ${:.2} ({:+.1}% MoM, z-score {:.2}, change score {:.2}, confidence {:.0}%)",
                service_name,
                metrics.anomaly_type,
                current_cost,
                metrics.baseline_cost,
                metrics.percent_change,
                metrics.z_score,
                metrics.change_point_score,
                (metrics.confidence * 100.0).round()
            );

            anomalies.push(CostAnomaly {
                service_name: service_name.clone(),
                anomaly_type: metrics.anomaly_type.clone(),
                severity: metrics.severity.clone(),
                baseline_cost: metrics.baseline_cost,
                actual_cost: current_cost,
                percentage_change: metrics.percent_change,
                description,
                z_score: Some(metrics.z_score),
                change_point_score: Some(metrics.change_point_score),
                trend_slope: Some(metrics.trend_slope),
                rolling_mean: Some(metrics.rolling_mean),
                rolling_std_dev: Some(metrics.rolling_std_dev),
                month_over_month_change: Some(metrics.month_over_month_change),
                month_over_month_percent: Some(metrics.percent_change),
                data_points_analyzed: Some(metrics.data_points),
                confidence: Some(metrics.confidence),
                seasonality_ratio: metrics.seasonality_ratio,
                baseline_mean: Some(metrics.baseline_mean),
                baseline_std_dev: Some(metrics.baseline_std_dev),
                detection_methods,
            });
        }

        Ok(anomalies)
    }

    fn evaluate_advanced_anomaly(
        &self,
        history: &[(NaiveDate, f64)],
        current_cost: f64,
    ) -> AdvancedAnomalyMetrics {
        AdvancedAnomalyMetrics::from_history(history, current_cost)
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

        // Generate LLM insights
        // Get the first active LLM provider
        let providers = self.llm_provider_repo.find_active().await?;
        if providers.is_empty() {
            tracing::warn!("No active LLM providers found, skipping insight generation for anomaly {}", anomaly.id);
            return Ok(());
        }
        let provider = &providers[0];

        let llm_request = crate::services::llm::LlmRequest {
            prompt: prompt.clone(),
            system_prompt: Some("You are a cost optimization expert. Analyze the cost anomaly and provide structured insights in JSON format with 'summary' and 'recommendations' fields.".to_string()),
            temperature: Some(0.3),
            max_tokens: Some(1000),
            variables: None,
        };

        match self.llm_service.generate_response(provider.id, llm_request).await {
            Ok(response) => {
                tracing::info!(
                    "Generated LLM insights for anomaly {} in service {}: {}",
                    anomaly.id,
                    anomaly.service_name,
                    response.content
                );

                // Parse the JSON response and store insights
                if let Ok(insights) = serde_json::from_str::<serde_json::Value>(&response.content) {
                    // Store the insights in the database
                    let insight_model = CostInsightActiveModel {
                        id: ActiveValue::NotSet,
                        anomaly_id: ActiveValue::Set(Some(anomaly.id)),
                        aggregate_id: ActiveValue::Set(Some(aggregate.id)),
                        account_id: ActiveValue::Set(aggregate.account_id.clone()),
                        insight_type: ActiveValue::Set("anomaly_analysis".to_string()),
                        prompt_template: ActiveValue::Set(prompt.clone()),
                        llm_provider: ActiveValue::Set(response.provider),
                        llm_model: ActiveValue::Set(response.model),
                        llm_response: ActiveValue::Set(response.content.clone()),
                        summary: ActiveValue::Set(
                            insights.get("summary")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        ),
                        recommendations: ActiveValue::Set(
                            insights.get("recommendations")
                                .and_then(|v| v.as_array())
                                .map(|arr| serde_json::Value::Array(arr.clone()))
                        ),
                        confidence_score: ActiveValue::Set(Some(Decimal::from(8) / Decimal::from(10))), // Default confidence
                        tokens_used: ActiveValue::Set(response.tokens_used.map(|t| t as i32)),
                        processing_time_ms: ActiveValue::Set(None), // Not available in LlmResponse
                        created_at: ActiveValue::Set(Utc::now().into()),
                    };

                    self.repository.create_cost_insight(insight_model).await?;
                } else {
                    tracing::warn!(
                        "Failed to parse LLM response as JSON for anomaly {}: {}",
                        anomaly.id,
                        response.content
                    );
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to generate LLM insights for anomaly {}: {}",
                    anomaly.id,
                    e
                );
                // Continue without LLM insights - don't fail the entire process
            }
        }

        Ok(())
    }

    /// Get cost summary for dashboard
    pub async fn get_cost_summary(&self, account_id: &str) -> Result<serde_json::Value, AppError> {
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

    /// Forecast future costs using historical data and linear regression
    pub async fn forecast_costs(
        &self,
        account_id: &str,
        months_ahead: u32,
        service_filter: Option<&str>,
    ) -> Result<serde_json::Value, AppError> {
        // Get historical data (last 12 months)
        let historical_data = self.repository
            .get_monthly_aggregates_by_account(account_id, Some(12))
            .await?;

        if historical_data.is_empty() {
            return Err(AppError::NotFound("Insufficient historical data for forecasting".to_string()));
        }

        // Filter by service if specified
        let filtered_data: Vec<_> = if let Some(service) = service_filter {
            historical_data.into_iter()
                .filter(|agg| agg.service_name == service)
                .collect()
        } else {
            historical_data
        };

        if filtered_data.is_empty() {
            return Err(AppError::NotFound(format!("No data found for service: {}", service_filter.unwrap_or("all"))));
        }

        // Sort by date
        let mut sorted_data = filtered_data;
        sorted_data.sort_by(|a, b| a.month_year.cmp(&b.month_year));

        // Prepare data for linear regression
        let mut x_values = Vec::new();
        let mut y_values = Vec::new();

        for (i, data_point) in sorted_data.iter().enumerate() {
            x_values.push(i as f64);
            y_values.push(data_point.total_cost.to_string().parse::<f64>().unwrap_or(0.0));
        }

        // Calculate linear regression
        let forecast = self.calculate_linear_regression(&x_values, &y_values, months_ahead)?;

        // Calculate confidence intervals and trends
        let trend_analysis = self.analyze_trend(&y_values)?;

        Ok(serde_json::json!({
            "forecast": forecast,
            "trend_analysis": trend_analysis,
            "historical_data_points": sorted_data.len(),
            "service_filter": service_filter,
            "forecast_period_months": months_ahead
        }))
    }

    /// Calculate linear regression for forecasting
    fn calculate_linear_regression(
        &self,
        x_values: &[f64],
        y_values: &[f64],
        months_ahead: u32,
    ) -> Result<serde_json::Value, AppError> {
        let n = x_values.len() as f64;

        if n < 2.0 {
            return Err(AppError::BadRequest("Need at least 2 data points for forecasting".to_string()));
        }

        // Calculate means
        let x_mean = x_values.iter().sum::<f64>() / n;
        let y_mean = y_values.iter().sum::<f64>() / n;

        // Calculate slope (m) and intercept (b) for y = mx + b
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (x, y) in x_values.iter().zip(y_values.iter()) {
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean) * (x - x_mean);
        }

        if denominator == 0.0 {
            return Err(AppError::BadRequest("Cannot calculate regression: division by zero".to_string()));
        }

        let slope = numerator / denominator;
        let intercept = y_mean - slope * x_mean;

        // Generate forecasts
        let mut forecasts = Vec::new();
        let last_x = *x_values.last().unwrap();

        for i in 1..=months_ahead {
            let future_x = last_x + i as f64;
            let predicted_cost = slope * future_x + intercept;
            let confidence_interval = self.calculate_confidence_interval(predicted_cost, n);

            forecasts.push(serde_json::json!({
                "month": i,
                "predicted_cost": predicted_cost.max(0.0), // Ensure non-negative costs
                "confidence_lower": (predicted_cost - confidence_interval).max(0.0),
                "confidence_upper": predicted_cost + confidence_interval,
                "trend_direction": if slope > 0.0 { "increasing" } else if slope < 0.0 { "decreasing" } else { "stable" }
            }));
        }

        Ok(serde_json::json!({
            "slope": slope,
            "intercept": intercept,
            "r_squared": self.calculate_r_squared(x_values, y_values, slope, intercept)?,
            "forecasts": forecasts
        }))
    }

    /// Calculate R-squared for regression quality
    fn calculate_r_squared(
        &self,
        x_values: &[f64],
        y_values: &[f64],
        slope: f64,
        intercept: f64,
    ) -> Result<f64, AppError> {
        let n = x_values.len() as f64;
        let y_mean = y_values.iter().sum::<f64>() / n;

        let mut ss_res = 0.0; // Sum of squares of residuals
        let mut ss_tot = 0.0; // Total sum of squares

        for (x, y) in x_values.iter().zip(y_values.iter()) {
            let predicted = slope * x + intercept;
            ss_res += (y - predicted).powi(2);
            ss_tot += (y - y_mean).powi(2);
        }

        if ss_tot == 0.0 {
            return Ok(0.0); // All values are the same
        }

        Ok(1.0 - (ss_res / ss_tot))
    }

    /// Calculate confidence interval for predictions
    fn calculate_confidence_interval(&self, predicted_value: f64, sample_size: f64) -> f64 {
        // Simple confidence interval calculation (simplified version)
        // In a real implementation, you'd use t-distribution, but this gives a reasonable approximation
        let standard_error = predicted_value * 0.1; // 10% standard error as approximation
        let confidence_level = 1.96; // 95% confidence interval
        confidence_level * standard_error / sample_size.sqrt()
    }

    /// Analyze trend in the data
    fn analyze_trend(&self, y_values: &[f64]) -> Result<serde_json::Value, AppError> {
        if y_values.len() < 2 {
            return Ok(serde_json::json!({
                "trend": "insufficient_data",
                "description": "Need at least 2 data points to analyze trend"
            }));
        }

        let first_value = y_values[0];
        let last_value = *y_values.last().unwrap();
        let total_change = last_value - first_value;
        let total_periods = (y_values.len() - 1) as f64;
        let average_change_per_period = total_change / total_periods;
        let percentage_change = if first_value != 0.0 {
            (total_change / first_value) * 100.0
        } else {
            0.0
        };

        // Calculate volatility (standard deviation of changes)
        let mut changes = Vec::new();
        for i in 1..y_values.len() {
            changes.push(y_values[i] - y_values[i-1]);
        }

        let avg_change = changes.iter().sum::<f64>() / changes.len() as f64;
        let variance = changes.iter()
            .map(|change| (change - avg_change).powi(2))
            .sum::<f64>() / changes.len() as f64;
        let volatility = variance.sqrt();

        let trend_direction = if average_change_per_period > 0.0 {
            "increasing"
        } else if average_change_per_period < 0.0 {
            "decreasing"
        } else {
            "stable"
        };

        let trend_strength = if percentage_change.abs() > 50.0 {
            "strong"
        } else if percentage_change.abs() > 20.0 {
            "moderate"
        } else {
            "weak"
        };

        Ok(serde_json::json!({
            "trend_direction": trend_direction,
            "trend_strength": trend_strength,
            "total_change": total_change,
            "percentage_change": percentage_change,
            "average_monthly_change": average_change_per_period,
            "volatility": volatility,
            "periods_analyzed": y_values.len(),
            "description": format!(
                "Cost trend is {} with {} change of {:.2}% over {} months",
                trend_direction, trend_strength, percentage_change, total_periods as u32
            )
        }))
    }
}
