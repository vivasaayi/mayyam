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

use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use aws_sdk_computeoptimizer::types::{
    AutoScalingGroupRecommendation, RecommendationSummary, SavingsOpportunity, Tag,
};
use aws_smithy_types::date_time::Format;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct ComputeOptimizerControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Debug, Default)]
struct SummaryAggregates {
    recommendation_count: usize,
    optimized_resource_count: usize,
    not_optimized_resource_count: usize,
    over_provisioned_resource_count: usize,
    under_provisioned_resource_count: usize,
    idle_resource_count: usize,
    high_performance_risk_count: i64,
    medium_performance_risk_count: i64,
    estimated_monthly_savings: f64,
}

#[derive(Debug, Default)]
struct RecommendationSamples {
    samples: Vec<Value>,
    sampled_recommendation_count: usize,
    sampled_optimized_count: usize,
    sampled_not_optimized_count: usize,
    sampled_over_provisioned_count: usize,
    sampled_under_provisioned_count: usize,
    sampled_unavailable_count: usize,
    sampled_high_performance_risk_count: usize,
    sampled_medium_performance_risk_count: usize,
    collection_error_count: usize,
}

impl ComputeOptimizerControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_accounts(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Compute Optimizer account inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_computeoptimizer_client(aws_account_dto)
            .await?;
        let enrollment = client.get_enrollment_status().send().await.map_err(|e| {
            error!(
                "Failed to get AWS Compute Optimizer enrollment status: {}",
                e
            );
            AppError::ExternalService(format!(
                "Failed to get AWS Compute Optimizer enrollment status: {}",
                e
            ))
        })?;
        let summaries = list_recommendation_summaries(&client, &aws_account_dto.account_id)
            .await
            .map_err(|e| {
                error!("Failed to list AWS Compute Optimizer summaries: {}", e);
                AppError::ExternalService(format!(
                    "Failed to list AWS Compute Optimizer summaries: {}",
                    e
                ))
            })?;
        let samples = collect_recommendation_samples(&client, &aws_account_dto.account_id).await;

        let resource = account_resource(aws_account_dto, sync_id, &enrollment, summaries, samples);

        debug!(
            "Successfully synced AWS Compute Optimizer account inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        Ok(vec![resource])
    }
}

async fn list_recommendation_summaries(
    client: &aws_sdk_computeoptimizer::Client,
    account_id: &str,
) -> Result<Vec<RecommendationSummary>, String> {
    let mut summaries = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .get_recommendation_summaries()
            .account_ids(account_id)
            .max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        summaries.extend(response.recommendation_summaries().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(summaries)
}

async fn collect_recommendation_samples(
    client: &aws_sdk_computeoptimizer::Client,
    account_id: &str,
) -> RecommendationSamples {
    let mut samples = RecommendationSamples::default();

    collect_ec2_samples(client, account_id, &mut samples).await;
    collect_asg_samples(client, account_id, &mut samples).await;
    collect_ebs_samples(client, account_id, &mut samples).await;
    collect_lambda_samples(client, account_id, &mut samples).await;

    samples
}

async fn collect_ec2_samples(
    client: &aws_sdk_computeoptimizer::Client,
    account_id: &str,
    samples: &mut RecommendationSamples,
) {
    let mut next_token: Option<String> = None;
    loop {
        let mut request = client
            .get_ec2_instance_recommendations()
            .account_ids(account_id)
            .max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Compute Optimizer EC2 recommendations for account {}: {}",
                    account_id, e
                );
                samples.collection_error_count += 1;
                break;
            }
        };

        samples.collection_error_count += response.errors().len();
        for recommendation in response.instance_recommendations() {
            let finding = recommendation.finding().map(|f| f.as_str());
            let risk = recommendation
                .current_performance_risk()
                .map(|r| r.as_str());
            record_sample_finding(samples, finding, risk);
            push_sample(
                samples,
                json!({
                    "resource_family": "ec2_instance",
                    "arn": recommendation.instance_arn(),
                    "account_id": recommendation.account_id(),
                    "name": recommendation.instance_name(),
                    "current_instance_type": recommendation.current_instance_type(),
                    "finding": finding,
                    "finding_reason_codes": recommendation.finding_reason_codes().iter().map(|reason| json!(reason.as_str())).collect::<Vec<Value>>(),
                    "current_performance_risk": risk,
                    "last_refresh_timestamp": fmt_date(recommendation.last_refresh_timestamp()),
                    "look_back_period_in_days": recommendation.look_back_period_in_days(),
                    "tags": tags_to_value(recommendation.tags()),
                }),
            );
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }
}

async fn collect_asg_samples(
    client: &aws_sdk_computeoptimizer::Client,
    account_id: &str,
    samples: &mut RecommendationSamples,
) {
    let mut next_token: Option<String> = None;
    loop {
        let mut request = client
            .get_auto_scaling_group_recommendations()
            .account_ids(account_id)
            .max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Compute Optimizer Auto Scaling recommendations for account {}: {}",
                    account_id, e
                );
                samples.collection_error_count += 1;
                break;
            }
        };

        samples.collection_error_count += response.errors().len();
        for recommendation in response.auto_scaling_group_recommendations() {
            let finding = recommendation.finding().map(|f| f.as_str());
            let risk = recommendation
                .current_performance_risk()
                .map(|r| r.as_str());
            record_sample_finding(samples, finding, risk);
            push_sample(
                samples,
                asg_recommendation_to_value(recommendation, finding, risk),
            );
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }
}

async fn collect_ebs_samples(
    client: &aws_sdk_computeoptimizer::Client,
    account_id: &str,
    samples: &mut RecommendationSamples,
) {
    let mut next_token: Option<String> = None;
    loop {
        let mut request = client
            .get_ebs_volume_recommendations()
            .account_ids(account_id)
            .max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Compute Optimizer EBS recommendations for account {}: {}",
                    account_id, e
                );
                samples.collection_error_count += 1;
                break;
            }
        };

        samples.collection_error_count += response.errors().len();
        for recommendation in response.volume_recommendations() {
            let finding = recommendation.finding().map(|f| f.as_str());
            let risk = recommendation
                .current_performance_risk()
                .map(|r| r.as_str());
            record_sample_finding(samples, finding, risk);
            push_sample(
                samples,
                json!({
                    "resource_family": "ebs_volume",
                    "arn": recommendation.volume_arn(),
                    "account_id": recommendation.account_id(),
                    "finding": finding,
                    "current_performance_risk": risk,
                    "last_refresh_timestamp": fmt_date(recommendation.last_refresh_timestamp()),
                    "look_back_period_in_days": recommendation.look_back_period_in_days(),
                    "tags": tags_to_value(recommendation.tags()),
                }),
            );
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }
}

async fn collect_lambda_samples(
    client: &aws_sdk_computeoptimizer::Client,
    account_id: &str,
    samples: &mut RecommendationSamples,
) {
    let mut next_token: Option<String> = None;
    loop {
        let mut request = client
            .get_lambda_function_recommendations()
            .account_ids(account_id)
            .max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Compute Optimizer Lambda recommendations for account {}: {}",
                    account_id, e
                );
                samples.collection_error_count += 1;
                break;
            }
        };

        for recommendation in response.lambda_function_recommendations() {
            let finding = recommendation.finding().map(|f| f.as_str());
            let risk = recommendation
                .current_performance_risk()
                .map(|r| r.as_str());
            record_sample_finding(samples, finding, risk);
            push_sample(
                samples,
                json!({
                    "resource_family": "lambda_function",
                    "arn": recommendation.function_arn(),
                    "account_id": recommendation.account_id(),
                    "function_version": recommendation.function_version(),
                    "current_memory_size": recommendation.current_memory_size(),
                    "number_of_invocations": recommendation.number_of_invocations(),
                    "finding": finding,
                    "finding_reason_codes": recommendation.finding_reason_codes().iter().map(|reason| json!(reason.as_str())).collect::<Vec<Value>>(),
                    "current_performance_risk": risk,
                    "last_refresh_timestamp": fmt_date(recommendation.last_refresh_timestamp()),
                    "look_back_period_in_days": recommendation.lookback_period_in_days(),
                    "tags": tags_to_value(recommendation.tags()),
                }),
            );
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }
}

fn account_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    enrollment: &aws_sdk_computeoptimizer::operation::get_enrollment_status::GetEnrollmentStatusOutput,
    summaries: Vec<RecommendationSummary>,
    samples: RecommendationSamples,
) -> AwsResourceModel {
    let aggregates = summarize_recommendations(&summaries);
    let summaries_json: Vec<Value> = summaries.iter().map(summary_to_value).collect();
    let enrollment_status = enrollment.status().map(|status| status.as_str());
    let arn = fallback_account_arn(aws_account_dto);

    let mut resource_data = serde_json::Map::new();
    resource_data.insert("asset_kind".to_string(), json!("account"));
    resource_data.insert("account_id".to_string(), json!(&aws_account_dto.account_id));
    resource_data.insert("enrollment_status".to_string(), json!(enrollment_status));
    resource_data.insert(
        "enrollment_status_reason".to_string(),
        json!(enrollment.status_reason()),
    );
    resource_data.insert(
        "recommendation_summary_count".to_string(),
        json!(summaries.len()),
    );
    resource_data.insert(
        "recommendation_count".to_string(),
        json!(aggregates.recommendation_count),
    );
    resource_data.insert(
        "optimized_resource_count".to_string(),
        json!(aggregates.optimized_resource_count),
    );
    resource_data.insert(
        "not_optimized_resource_count".to_string(),
        json!(aggregates.not_optimized_resource_count),
    );
    resource_data.insert(
        "over_provisioned_resource_count".to_string(),
        json!(aggregates.over_provisioned_resource_count),
    );
    resource_data.insert(
        "under_provisioned_resource_count".to_string(),
        json!(aggregates.under_provisioned_resource_count),
    );
    resource_data.insert(
        "idle_resource_count".to_string(),
        json!(aggregates.idle_resource_count),
    );
    resource_data.insert(
        "high_performance_risk_count".to_string(),
        json!(aggregates.high_performance_risk_count),
    );
    resource_data.insert(
        "medium_performance_risk_count".to_string(),
        json!(aggregates.medium_performance_risk_count),
    );
    resource_data.insert(
        "estimated_monthly_savings".to_string(),
        json!(aggregates.estimated_monthly_savings),
    );
    resource_data.insert(
        "sampled_recommendation_count".to_string(),
        json!(samples.sampled_recommendation_count),
    );
    resource_data.insert(
        "sampled_optimized_count".to_string(),
        json!(samples.sampled_optimized_count),
    );
    resource_data.insert(
        "sampled_not_optimized_count".to_string(),
        json!(samples.sampled_not_optimized_count),
    );
    resource_data.insert(
        "sampled_over_provisioned_count".to_string(),
        json!(samples.sampled_over_provisioned_count),
    );
    resource_data.insert(
        "sampled_under_provisioned_count".to_string(),
        json!(samples.sampled_under_provisioned_count),
    );
    resource_data.insert(
        "sampled_unavailable_count".to_string(),
        json!(samples.sampled_unavailable_count),
    );
    resource_data.insert(
        "sampled_high_performance_risk_count".to_string(),
        json!(samples.sampled_high_performance_risk_count),
    );
    resource_data.insert(
        "sampled_medium_performance_risk_count".to_string(),
        json!(samples.sampled_medium_performance_risk_count),
    );
    resource_data.insert(
        "recommendation_collection_error_count".to_string(),
        json!(samples.collection_error_count),
    );
    resource_data.insert(
        "recommendation_summaries".to_string(),
        Value::Array(summaries_json),
    );
    resource_data.insert(
        "sampled_recommendations".to_string(),
        Value::Array(samples.samples),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::ComputeOptimizerAccount.to_string(),
        resource_id: format!("computeoptimizer:{}", aws_account_dto.account_id),
        arn,
        name: Some("Compute Optimizer".to_string()),
        tags: json!({}),
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn summarize_recommendations(summaries: &[RecommendationSummary]) -> SummaryAggregates {
    let mut aggregates = SummaryAggregates::default();

    for summary in summaries {
        for finding in summary.summaries() {
            let count = count_value(finding.value());
            aggregates.recommendation_count += count;
            match finding.name().map(|name| name.as_str()) {
                Some("Optimized") => aggregates.optimized_resource_count += count,
                Some("NotOptimized") => aggregates.not_optimized_resource_count += count,
                Some("Overprovisioned") => aggregates.over_provisioned_resource_count += count,
                Some("Underprovisioned") => aggregates.under_provisioned_resource_count += count,
                _ => {}
            }
        }

        for idle in summary.idle_summaries() {
            let count = count_value(idle.value());
            aggregates.recommendation_count += count;
            aggregates.idle_resource_count += count;
        }

        if let Some(risk) = summary.current_performance_risk_ratings() {
            aggregates.high_performance_risk_count += risk.high();
            aggregates.medium_performance_risk_count += risk.medium();
        }

        aggregates.estimated_monthly_savings += summary_savings(summary);
    }

    aggregates
}

fn summary_savings(summary: &RecommendationSummary) -> f64 {
    if let Some(aggregated) = summary.aggregated_savings_opportunity() {
        return savings_value(aggregated);
    }

    savings_value_opt(summary.savings_opportunity())
        + savings_value_opt(summary.idle_savings_opportunity())
}

fn savings_value_opt(savings: Option<&SavingsOpportunity>) -> f64 {
    savings.map(savings_value).unwrap_or(0.0)
}

fn savings_value(savings: &SavingsOpportunity) -> f64 {
    savings
        .estimated_monthly_savings()
        .map(|monthly| monthly.value())
        .unwrap_or(0.0)
}

fn count_value(value: f64) -> usize {
    if value.is_finite() && value > 0.0 {
        value.round() as usize
    } else {
        0
    }
}

fn record_sample_finding(
    samples: &mut RecommendationSamples,
    finding: Option<&str>,
    performance_risk: Option<&str>,
) {
    samples.sampled_recommendation_count += 1;

    match finding {
        Some("Optimized") => samples.sampled_optimized_count += 1,
        Some("NotOptimized") => samples.sampled_not_optimized_count += 1,
        Some("Overprovisioned") => samples.sampled_over_provisioned_count += 1,
        Some("Underprovisioned") => samples.sampled_under_provisioned_count += 1,
        Some("Unavailable") => samples.sampled_unavailable_count += 1,
        _ => {}
    }

    match performance_risk {
        Some("High") => samples.sampled_high_performance_risk_count += 1,
        Some("Medium") => samples.sampled_medium_performance_risk_count += 1,
        _ => {}
    }
}

fn push_sample(samples: &mut RecommendationSamples, sample: Value) {
    if samples.samples.len() < 100 {
        samples.samples.push(sample);
    }
}

fn summary_to_value(summary: &RecommendationSummary) -> Value {
    json!({
        "account_id": summary.account_id(),
        "resource_type": summary.recommendation_resource_type().map(|resource_type| resource_type.as_str()),
        "summaries": summary.summaries().iter().map(finding_summary_to_value).collect::<Vec<Value>>(),
        "idle_summaries": summary.idle_summaries().iter().map(|idle| {
            json!({
                "name": idle.name().map(|name| name.as_str()),
                "value": idle.value(),
            })
        }).collect::<Vec<Value>>(),
        "savings_opportunity": summary.savings_opportunity().map(savings_to_value),
        "idle_savings_opportunity": summary.idle_savings_opportunity().map(savings_to_value),
        "aggregated_savings_opportunity": summary.aggregated_savings_opportunity().map(savings_to_value),
        "current_performance_risk_ratings": summary.current_performance_risk_ratings().map(|risk| {
            json!({
                "high": risk.high(),
                "medium": risk.medium(),
                "low": risk.low(),
                "very_low": risk.very_low(),
            })
        }),
    })
}

fn finding_summary_to_value(summary: &aws_sdk_computeoptimizer::types::Summary) -> Value {
    json!({
        "name": summary.name().map(|name| name.as_str()),
        "value": summary.value(),
        "reason_code_summaries": summary.reason_code_summaries().iter().map(|reason| {
            json!({
                "name": reason.name().map(|name| name.as_str()),
                "value": reason.value(),
            })
        }).collect::<Vec<Value>>(),
    })
}

fn savings_to_value(savings: &SavingsOpportunity) -> Value {
    json!({
        "savings_opportunity_percentage": savings.savings_opportunity_percentage(),
        "estimated_monthly_savings": savings.estimated_monthly_savings().map(|monthly| {
            json!({
                "currency": monthly.currency().map(|currency| currency.as_str()),
                "value": monthly.value(),
            })
        }),
    })
}

fn asg_recommendation_to_value(
    recommendation: &AutoScalingGroupRecommendation,
    finding: Option<&str>,
    risk: Option<&str>,
) -> Value {
    json!({
        "resource_family": "auto_scaling_group",
        "arn": recommendation.auto_scaling_group_arn(),
        "account_id": recommendation.account_id(),
        "name": recommendation.auto_scaling_group_name(),
        "finding": finding,
        "current_performance_risk": risk,
        "last_refresh_timestamp": fmt_date(recommendation.last_refresh_timestamp()),
        "look_back_period_in_days": recommendation.look_back_period_in_days(),
    })
}

fn tags_to_value(tags: &[Tag]) -> Value {
    Value::Array(
        tags.iter()
            .map(|tag| {
                json!({
                    "key": tag.key(),
                    "value": tag.value(),
                })
            })
            .collect(),
    )
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.map(|d| {
        d.fmt(Format::DateTime)
            .unwrap_or_else(|_| format!("{:?}", d))
    })
}

fn fallback_account_arn(aws_account_dto: &AwsAccountDto) -> String {
    format!(
        "arn:aws:compute-optimizer:{}:{}:account/{}",
        aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
    )
}
