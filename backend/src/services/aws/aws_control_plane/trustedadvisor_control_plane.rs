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
use aws_sdk_trustedadvisor::types::{
    CheckSummary, RecommendationLanguage, RecommendationPillar, RecommendationResourceSummary,
    RecommendationSummary,
};
use aws_smithy_types::date_time::Format;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct TrustedAdvisorControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Debug, Default)]
struct ResourceSamples {
    samples: Vec<Value>,
    sampled_resource_count: usize,
    warning_resource_sample_count: usize,
    error_resource_sample_count: usize,
    excluded_resource_sample_count: usize,
    collection_error_count: usize,
}

#[derive(Debug, Default)]
struct RecommendationAggregates {
    ok_recommendation_count: usize,
    warning_recommendation_count: usize,
    error_recommendation_count: usize,
    cost_recommendation_count: usize,
    cost_warning_or_error_count: usize,
    resilience_recommendation_count: usize,
    resilience_warning_or_error_count: usize,
    service_limit_warning_or_error_count: usize,
    security_recommendation_count: usize,
    security_warning_or_error_count: usize,
    ok_resource_count: i64,
    warning_resource_count: i64,
    error_resource_count: i64,
    excluded_resource_count: i64,
    estimated_monthly_savings: f64,
}

impl TrustedAdvisorControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_accounts(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Trusted Advisor account inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_trustedadvisor_client(aws_account_dto)
            .await?;
        let checks = list_checks(&client).await.map_err(|e| {
            error!("Failed to list AWS Trusted Advisor checks: {}", e);
            AppError::ExternalService(format!("Failed to list AWS Trusted Advisor checks: {}", e))
        })?;
        let recommendations = list_recommendations(&client).await.map_err(|e| {
            error!("Failed to list AWS Trusted Advisor recommendations: {}", e);
            AppError::ExternalService(format!(
                "Failed to list AWS Trusted Advisor recommendations: {}",
                e
            ))
        })?;
        let resource_samples = collect_resource_samples(&client, &recommendations).await;

        let resource = account_resource(
            aws_account_dto,
            sync_id,
            checks,
            recommendations,
            resource_samples,
        );

        debug!(
            "Successfully synced AWS Trusted Advisor account inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(vec![resource])
    }
}

async fn list_checks(client: &aws_sdk_trustedadvisor::Client) -> Result<Vec<CheckSummary>, String> {
    let mut checks = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_checks()
            .language(RecommendationLanguage::English)
            .max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        checks.extend(response.check_summaries().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(checks)
}

async fn list_recommendations(
    client: &aws_sdk_trustedadvisor::Client,
) -> Result<Vec<RecommendationSummary>, String> {
    let mut recommendations = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_recommendations()
            .language(RecommendationLanguage::English)
            .max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        recommendations.extend(response.recommendation_summaries().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(recommendations)
}

async fn collect_resource_samples(
    client: &aws_sdk_trustedadvisor::Client,
    recommendations: &[RecommendationSummary],
) -> ResourceSamples {
    let mut summary = ResourceSamples::default();

    for recommendation in recommendations {
        let mut next_token: Option<String> = None;
        loop {
            let mut request = client
                .list_recommendation_resources()
                .recommendation_identifier(recommendation.id())
                .language(RecommendationLanguage::English)
                .max_results(100);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "Failed to list AWS Trusted Advisor resources for recommendation {}: {}",
                        recommendation.id(),
                        e
                    );
                    summary.collection_error_count += 1;
                    break;
                }
            };

            for resource in response.recommendation_resource_summaries() {
                summary.sampled_resource_count += 1;
                match resource.status().as_str() {
                    "warning" => summary.warning_resource_sample_count += 1,
                    "error" => summary.error_resource_sample_count += 1,
                    _ => {}
                }
                if resource.exclusion_status().as_str() == "excluded" {
                    summary.excluded_resource_sample_count += 1;
                }
                if summary.samples.len() < 100 {
                    summary.samples.push(resource_to_value(resource));
                }
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }
    }

    summary
}

fn account_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    checks: Vec<CheckSummary>,
    recommendations: Vec<RecommendationSummary>,
    resource_samples: ResourceSamples,
) -> AwsResourceModel {
    let aggregates = summarize_recommendations(&recommendations);
    let arn = fallback_account_arn(aws_account_dto);
    let checks_json: Vec<Value> = checks.iter().map(check_to_value).collect();
    let recommendations_json: Vec<Value> = recommendations
        .iter()
        .map(recommendation_to_value)
        .collect();

    let mut resource_data = serde_json::Map::new();
    resource_data.insert("asset_kind".to_string(), json!("account"));
    resource_data.insert("account_id".to_string(), json!(&aws_account_dto.account_id));
    resource_data.insert("check_count".to_string(), json!(checks.len()));
    resource_data.insert(
        "recommendation_count".to_string(),
        json!(recommendations.len()),
    );
    resource_data.insert(
        "ok_recommendation_count".to_string(),
        json!(aggregates.ok_recommendation_count),
    );
    resource_data.insert(
        "warning_recommendation_count".to_string(),
        json!(aggregates.warning_recommendation_count),
    );
    resource_data.insert(
        "error_recommendation_count".to_string(),
        json!(aggregates.error_recommendation_count),
    );
    resource_data.insert(
        "cost_recommendation_count".to_string(),
        json!(aggregates.cost_recommendation_count),
    );
    resource_data.insert(
        "cost_warning_or_error_count".to_string(),
        json!(aggregates.cost_warning_or_error_count),
    );
    resource_data.insert(
        "resilience_recommendation_count".to_string(),
        json!(aggregates.resilience_recommendation_count),
    );
    resource_data.insert(
        "resilience_warning_or_error_count".to_string(),
        json!(aggregates.resilience_warning_or_error_count),
    );
    resource_data.insert(
        "service_limit_warning_or_error_count".to_string(),
        json!(aggregates.service_limit_warning_or_error_count),
    );
    resource_data.insert(
        "security_recommendation_count".to_string(),
        json!(aggregates.security_recommendation_count),
    );
    resource_data.insert(
        "security_warning_or_error_count".to_string(),
        json!(aggregates.security_warning_or_error_count),
    );
    resource_data.insert(
        "ok_resource_count".to_string(),
        json!(aggregates.ok_resource_count),
    );
    resource_data.insert(
        "warning_resource_count".to_string(),
        json!(aggregates.warning_resource_count),
    );
    resource_data.insert(
        "error_resource_count".to_string(),
        json!(aggregates.error_resource_count),
    );
    resource_data.insert(
        "excluded_resource_count".to_string(),
        json!(aggregates.excluded_resource_count),
    );
    resource_data.insert(
        "estimated_monthly_savings".to_string(),
        json!(aggregates.estimated_monthly_savings),
    );
    resource_data.insert(
        "sampled_resource_count".to_string(),
        json!(resource_samples.sampled_resource_count),
    );
    resource_data.insert(
        "warning_resource_sample_count".to_string(),
        json!(resource_samples.warning_resource_sample_count),
    );
    resource_data.insert(
        "error_resource_sample_count".to_string(),
        json!(resource_samples.error_resource_sample_count),
    );
    resource_data.insert(
        "excluded_resource_sample_count".to_string(),
        json!(resource_samples.excluded_resource_sample_count),
    );
    resource_data.insert(
        "resource_collection_error_count".to_string(),
        json!(resource_samples.collection_error_count),
    );
    resource_data.insert("checks".to_string(), Value::Array(checks_json));
    resource_data.insert(
        "recommendations".to_string(),
        Value::Array(recommendations_json),
    );
    resource_data.insert(
        "sampled_resources".to_string(),
        Value::Array(resource_samples.samples),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::TrustedAdvisorAccount.to_string(),
        resource_id: format!("trustedadvisor:{}", aws_account_dto.account_id),
        arn,
        name: Some("Trusted Advisor".to_string()),
        tags: json!({}),
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn summarize_recommendations(
    recommendations: &[RecommendationSummary],
) -> RecommendationAggregates {
    let mut aggregates = RecommendationAggregates::default();

    for recommendation in recommendations {
        let has_cost = has_pillar(recommendation, "cost_optimizing");
        let has_fault_tolerance = has_pillar(recommendation, "fault_tolerance");
        let has_service_limits = has_pillar(recommendation, "service_limits");
        let has_resilience = has_fault_tolerance || has_service_limits;
        let has_security = has_pillar(recommendation, "security");
        let warning_or_error = matches!(recommendation.status().as_str(), "warning" | "error");

        match recommendation.status().as_str() {
            "ok" => aggregates.ok_recommendation_count += 1,
            "warning" => aggregates.warning_recommendation_count += 1,
            "error" => aggregates.error_recommendation_count += 1,
            _ => {}
        }

        if has_cost {
            aggregates.cost_recommendation_count += 1;
            if warning_or_error {
                aggregates.cost_warning_or_error_count += 1;
            }
        }
        if has_resilience {
            aggregates.resilience_recommendation_count += 1;
            if warning_or_error {
                aggregates.resilience_warning_or_error_count += 1;
            }
            if has_service_limits && warning_or_error {
                aggregates.service_limit_warning_or_error_count += 1;
            }
        }
        if has_security {
            aggregates.security_recommendation_count += 1;
            if warning_or_error {
                aggregates.security_warning_or_error_count += 1;
            }
        }

        if let Some(resources) = recommendation.resources_aggregates() {
            aggregates.ok_resource_count += resources.ok_count();
            aggregates.warning_resource_count += resources.warning_count();
            aggregates.error_resource_count += resources.error_count();
            aggregates.excluded_resource_count += resources.excluded_count().unwrap_or(0);
        }
        if let Some(cost) = recommendation
            .pillar_specific_aggregates()
            .and_then(|pillar| pillar.cost_optimizing())
        {
            aggregates.estimated_monthly_savings += cost.estimated_monthly_savings();
        }
    }

    aggregates
}

fn has_pillar(recommendation: &RecommendationSummary, pillar: &str) -> bool {
    recommendation
        .pillars()
        .iter()
        .any(|candidate| candidate.as_str() == pillar)
}

fn check_to_value(check: &CheckSummary) -> Value {
    json!({
        "id": check.id(),
        "arn": check.arn(),
        "name": check.name(),
        "description": check.description(),
        "pillars": pillars_to_value(check.pillars()),
        "aws_services": check.aws_services(),
        "source": check.source().as_str(),
        "metadata": check.metadata(),
    })
}

fn recommendation_to_value(recommendation: &RecommendationSummary) -> Value {
    json!({
        "id": recommendation.id(),
        "arn": recommendation.arn(),
        "check_arn": recommendation.check_arn(),
        "name": recommendation.name(),
        "type": recommendation.r#type().as_str(),
        "status": recommendation.status().as_str(),
        "lifecycle_stage": recommendation.lifecycle_stage().map(|stage| stage.as_str()),
        "pillars": pillars_to_value(recommendation.pillars()),
        "source": recommendation.source().as_str(),
        "aws_services": recommendation.aws_services(),
        "created_at": fmt_date(recommendation.created_at()),
        "last_updated_at": fmt_date(recommendation.last_updated_at()),
        "resources_aggregates": recommendation.resources_aggregates().map(resources_aggregates_to_value),
        "pillar_specific_aggregates": recommendation.pillar_specific_aggregates().map(pillar_aggregates_to_value),
        "status_reason": recommendation.status_reason().map(|reason| reason.as_str()),
    })
}

fn resource_to_value(resource: &RecommendationResourceSummary) -> Value {
    json!({
        "id": resource.id(),
        "arn": resource.arn(),
        "aws_resource_id": resource.aws_resource_id(),
        "region_code": resource.region_code(),
        "status": resource.status().as_str(),
        "metadata": resource.metadata(),
        "last_updated_at": fmt_date(Some(resource.last_updated_at())),
        "exclusion_status": resource.exclusion_status().as_str(),
        "recommendation_arn": resource.recommendation_arn(),
    })
}

fn pillars_to_value(pillars: &[RecommendationPillar]) -> Value {
    Value::Array(
        pillars
            .iter()
            .map(|pillar| json!(pillar.as_str()))
            .collect(),
    )
}

fn resources_aggregates_to_value(
    aggregates: &aws_sdk_trustedadvisor::types::RecommendationResourcesAggregates,
) -> Value {
    json!({
        "ok_count": aggregates.ok_count(),
        "warning_count": aggregates.warning_count(),
        "error_count": aggregates.error_count(),
        "excluded_count": aggregates.excluded_count(),
    })
}

fn pillar_aggregates_to_value(
    aggregates: &aws_sdk_trustedadvisor::types::RecommendationPillarSpecificAggregates,
) -> Value {
    json!({
        "cost_optimizing": aggregates.cost_optimizing().map(|cost| {
            json!({
                "estimated_monthly_savings": cost.estimated_monthly_savings(),
                "estimated_percent_monthly_savings": cost.estimated_percent_monthly_savings(),
            })
        }),
    })
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.map(|d| {
        d.fmt(Format::DateTime)
            .unwrap_or_else(|_| format!("{:?}", d))
    })
}

fn fallback_account_arn(aws_account_dto: &AwsAccountDto) -> String {
    format!(
        "arn:aws:trustedadvisor:{}:{}:account/{}",
        aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
    )
}
