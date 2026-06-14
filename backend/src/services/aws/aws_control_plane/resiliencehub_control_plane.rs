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
use aws_sdk_resiliencehub::types::{
    App, AppAssessmentSummary, AppComponentCompliance, AppSummary, Cost, DisruptionCompliance,
    FailurePolicy, ResiliencyPolicy,
};
use aws_smithy_types::date_time::Format;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

const LOW_RESILIENCY_SCORE_THRESHOLD: f64 = 70.0;
const COMPONENT_COMPLIANCE_ASSESSMENT_LIMIT: usize = 50;
const SAMPLE_LIMIT: usize = 100;

pub struct ResilienceHubControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Debug, Default)]
struct AppAggregates {
    noncompliant_app_count: usize,
    drifted_app_count: usize,
    low_resiliency_score_app_count: usize,
    daily_assessment_app_count: usize,
    disabled_assessment_app_count: usize,
}

#[derive(Debug, Default)]
struct AppDetailAggregates {
    app_detail_collection_error_count: usize,
    policy_linked_app_count: usize,
    app_with_tags_count: usize,
    event_subscription_count: usize,
}

#[derive(Debug, Default)]
struct AssessmentAggregates {
    assessment_collection_error_count: usize,
    noncompliant_assessment_count: usize,
    failed_assessment_count: usize,
    drifted_assessment_count: usize,
    low_resiliency_score_assessment_count: usize,
    estimated_cost_amount_total: f64,
}

#[derive(Debug, Default)]
struct ComponentComplianceAggregates {
    component_compliance_count: usize,
    noncompliant_component_count: usize,
    component_compliance_collection_error_count: usize,
    samples: Vec<Value>,
}

impl ResilienceHubControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_accounts(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Resilience Hub inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_resiliencehub_client(aws_account_dto)
            .await?;
        let apps = list_apps(&client).await.map_err(|e| {
            error!("Failed to list Resilience Hub apps: {}", e);
            AppError::ExternalService(format!("Failed to list Resilience Hub apps: {}", e))
        })?;
        let policies = list_resiliency_policies(&client).await.map_err(|e| {
            error!("Failed to list Resilience Hub policies: {}", e);
            AppError::ExternalService(format!("Failed to list Resilience Hub policies: {}", e))
        })?;
        let (app_details, app_detail_aggregates) = collect_app_details(&client, &apps).await;
        let (assessments, assessment_aggregates) = collect_assessments(&client, &apps).await;
        let component_compliances = collect_component_compliances(&client, &assessments).await;
        let resource = account_resource(
            aws_account_dto,
            sync_id,
            apps,
            policies,
            app_details,
            app_detail_aggregates,
            assessments,
            assessment_aggregates,
            component_compliances,
        );

        debug!(
            "Successfully synced AWS Resilience Hub inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        Ok(vec![resource])
    }
}

async fn list_apps(client: &aws_sdk_resiliencehub::Client) -> Result<Vec<AppSummary>, String> {
    let mut apps = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_apps().max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        apps.extend(response.app_summaries().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(apps)
}

async fn list_resiliency_policies(
    client: &aws_sdk_resiliencehub::Client,
) -> Result<Vec<ResiliencyPolicy>, String> {
    let mut policies = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_resiliency_policies().max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        policies.extend(response.resiliency_policies().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(policies)
}

async fn collect_app_details(
    client: &aws_sdk_resiliencehub::Client,
    apps: &[AppSummary],
) -> (Vec<App>, AppDetailAggregates) {
    let mut details = Vec::new();
    let mut aggregates = AppDetailAggregates::default();

    for app in apps {
        match client.describe_app().app_arn(app.app_arn()).send().await {
            Ok(response) => {
                if let Some(detail) = response.app().cloned() {
                    if detail.policy_arn().is_some() {
                        aggregates.policy_linked_app_count += 1;
                    }
                    if detail.tags().map(|tags| !tags.is_empty()).unwrap_or(false) {
                        aggregates.app_with_tags_count += 1;
                    }
                    aggregates.event_subscription_count += detail.event_subscriptions().len();
                    details.push(detail);
                }
            }
            Err(e) => {
                debug!(
                    "Failed to describe Resilience Hub app {}: {}",
                    app.app_arn(),
                    e
                );
                aggregates.app_detail_collection_error_count += 1;
            }
        }
    }

    (details, aggregates)
}

async fn collect_assessments(
    client: &aws_sdk_resiliencehub::Client,
    apps: &[AppSummary],
) -> (Vec<AppAssessmentSummary>, AssessmentAggregates) {
    let mut assessments = Vec::new();
    let mut aggregates = AssessmentAggregates::default();

    for app in apps {
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client
                .list_app_assessments()
                .app_arn(app.app_arn())
                .reverse_order(true)
                .max_results(100);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "Failed to list Resilience Hub assessments for app {}: {}",
                        app.app_arn(),
                        e
                    );
                    aggregates.assessment_collection_error_count += 1;
                    break;
                }
            };

            for assessment in response.assessment_summaries() {
                record_assessment(&mut aggregates, assessment);
                assessments.push(assessment.clone());
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }
    }

    (assessments, aggregates)
}

async fn collect_component_compliances(
    client: &aws_sdk_resiliencehub::Client,
    assessments: &[AppAssessmentSummary],
) -> ComponentComplianceAggregates {
    let mut aggregates = ComponentComplianceAggregates::default();

    for assessment in assessments
        .iter()
        .take(COMPONENT_COMPLIANCE_ASSESSMENT_LIMIT)
    {
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client
                .list_app_component_compliances()
                .assessment_arn(assessment.assessment_arn())
                .max_results(100);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "Failed to list Resilience Hub component compliance for assessment {}: {}",
                        assessment.assessment_arn(),
                        e
                    );
                    aggregates.component_compliance_collection_error_count += 1;
                    break;
                }
            };

            for component in response.component_compliances() {
                aggregates.component_compliance_count += 1;
                if component_is_noncompliant(component) {
                    aggregates.noncompliant_component_count += 1;
                }
                if aggregates.samples.len() < SAMPLE_LIMIT {
                    aggregates
                        .samples
                        .push(component_compliance_to_value(component));
                }
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }
    }

    aggregates
}

fn record_assessment(aggregates: &mut AssessmentAggregates, assessment: &AppAssessmentSummary) {
    if assessment
        .compliance_status()
        .map(compliance_is_noncompliant)
        .unwrap_or(false)
    {
        aggregates.noncompliant_assessment_count += 1;
    }
    if assessment.assessment_status().as_str() == "Failed" {
        aggregates.failed_assessment_count += 1;
    }
    if assessment
        .drift_status()
        .map(|status| status.as_str() == "Detected")
        .unwrap_or(false)
    {
        aggregates.drifted_assessment_count += 1;
    }
    if assessment.resiliency_score() < LOW_RESILIENCY_SCORE_THRESHOLD {
        aggregates.low_resiliency_score_assessment_count += 1;
    }
    if let Some(cost) = assessment.cost() {
        aggregates.estimated_cost_amount_total += cost.amount();
    }
}

fn account_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    apps: Vec<AppSummary>,
    policies: Vec<ResiliencyPolicy>,
    app_details: Vec<App>,
    app_detail_aggregates: AppDetailAggregates,
    assessments: Vec<AppAssessmentSummary>,
    assessment_aggregates: AssessmentAggregates,
    component_compliances: ComponentComplianceAggregates,
) -> AwsResourceModel {
    let app_aggregates = summarize_apps(&apps);
    let app_samples: Vec<Value> = apps
        .iter()
        .take(SAMPLE_LIMIT)
        .map(|app| app_to_value(app, find_app_detail(&app_details, app.app_arn())))
        .collect();
    let policy_samples: Vec<Value> = policies
        .iter()
        .take(SAMPLE_LIMIT)
        .map(policy_to_value)
        .collect();
    let assessment_samples: Vec<Value> = assessments
        .iter()
        .take(SAMPLE_LIMIT)
        .map(assessment_to_value)
        .collect();
    let arn = fallback_account_arn(aws_account_dto);

    let mut resource_data = serde_json::Map::new();
    resource_data.insert("asset_kind".to_string(), json!("account"));
    resource_data.insert("account_id".to_string(), json!(&aws_account_dto.account_id));
    resource_data.insert("app_count".to_string(), json!(apps.len()));
    resource_data.insert("policy_count".to_string(), json!(policies.len()));
    resource_data.insert("app_detail_count".to_string(), json!(app_details.len()));
    resource_data.insert(
        "app_detail_collection_error_count".to_string(),
        json!(app_detail_aggregates.app_detail_collection_error_count),
    );
    resource_data.insert(
        "policy_linked_app_count".to_string(),
        json!(app_detail_aggregates.policy_linked_app_count),
    );
    resource_data.insert(
        "app_with_tags_count".to_string(),
        json!(app_detail_aggregates.app_with_tags_count),
    );
    resource_data.insert(
        "event_subscription_count".to_string(),
        json!(app_detail_aggregates.event_subscription_count),
    );
    resource_data.insert(
        "daily_assessment_app_count".to_string(),
        json!(app_aggregates.daily_assessment_app_count),
    );
    resource_data.insert(
        "disabled_assessment_app_count".to_string(),
        json!(app_aggregates.disabled_assessment_app_count),
    );
    resource_data.insert(
        "noncompliant_app_count".to_string(),
        json!(app_aggregates.noncompliant_app_count),
    );
    resource_data.insert(
        "drifted_app_count".to_string(),
        json!(app_aggregates.drifted_app_count),
    );
    resource_data.insert(
        "low_resiliency_score_app_count".to_string(),
        json!(app_aggregates.low_resiliency_score_app_count),
    );
    resource_data.insert("assessment_count".to_string(), json!(assessments.len()));
    resource_data.insert(
        "assessment_collection_error_count".to_string(),
        json!(assessment_aggregates.assessment_collection_error_count),
    );
    resource_data.insert(
        "noncompliant_assessment_count".to_string(),
        json!(assessment_aggregates.noncompliant_assessment_count),
    );
    resource_data.insert(
        "failed_assessment_count".to_string(),
        json!(assessment_aggregates.failed_assessment_count),
    );
    resource_data.insert(
        "drifted_assessment_count".to_string(),
        json!(assessment_aggregates.drifted_assessment_count),
    );
    resource_data.insert(
        "low_resiliency_score_assessment_count".to_string(),
        json!(assessment_aggregates.low_resiliency_score_assessment_count),
    );
    resource_data.insert(
        "estimated_cost_amount_total".to_string(),
        json!(assessment_aggregates.estimated_cost_amount_total),
    );
    resource_data.insert(
        "component_compliance_count".to_string(),
        json!(component_compliances.component_compliance_count),
    );
    resource_data.insert(
        "noncompliant_component_count".to_string(),
        json!(component_compliances.noncompliant_component_count),
    );
    resource_data.insert(
        "component_compliance_collection_error_count".to_string(),
        json!(component_compliances.component_compliance_collection_error_count),
    );
    resource_data.insert(
        "component_compliance_assessment_scan_limit".to_string(),
        json!(COMPONENT_COMPLIANCE_ASSESSMENT_LIMIT),
    );
    resource_data.insert("app_sample_count".to_string(), json!(app_samples.len()));
    resource_data.insert(
        "policy_sample_count".to_string(),
        json!(policy_samples.len()),
    );
    resource_data.insert(
        "assessment_sample_count".to_string(),
        json!(assessment_samples.len()),
    );
    resource_data.insert(
        "component_compliance_sample_count".to_string(),
        json!(component_compliances.samples.len()),
    );
    resource_data.insert("apps".to_string(), Value::Array(app_samples));
    resource_data.insert(
        "resiliency_policies".to_string(),
        Value::Array(policy_samples),
    );
    resource_data.insert("assessments".to_string(), Value::Array(assessment_samples));
    resource_data.insert(
        "component_compliances".to_string(),
        Value::Array(component_compliances.samples),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::ResilienceHubAccount.to_string(),
        resource_id: format!("resiliencehub:{}", aws_account_dto.account_id),
        arn,
        name: Some("AWS Resilience Hub".to_string()),
        tags: json!({}),
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn summarize_apps(apps: &[AppSummary]) -> AppAggregates {
    let mut aggregates = AppAggregates::default();

    for app in apps {
        if app
            .compliance_status()
            .map(app_compliance_is_noncompliant)
            .unwrap_or(false)
        {
            aggregates.noncompliant_app_count += 1;
        }
        if app
            .drift_status()
            .map(|status| status.as_str() == "Detected")
            .unwrap_or(false)
        {
            aggregates.drifted_app_count += 1;
        }
        if app.resiliency_score() < LOW_RESILIENCY_SCORE_THRESHOLD {
            aggregates.low_resiliency_score_app_count += 1;
        }
        match app.assessment_schedule().map(|schedule| schedule.as_str()) {
            Some("Daily") => aggregates.daily_assessment_app_count += 1,
            Some("Disabled") => aggregates.disabled_assessment_app_count += 1,
            _ => {}
        }
    }

    aggregates
}

fn find_app_detail<'a>(app_details: &'a [App], app_arn: &str) -> Option<&'a App> {
    app_details.iter().find(|app| app.app_arn() == app_arn)
}

fn app_to_value(app: &AppSummary, detail: Option<&App>) -> Value {
    json!({
        "app_arn": app.app_arn(),
        "name": app.name(),
        "description": app.description(),
        "creation_time": format_date(Some(app.creation_time())),
        "status": app.status().map(|status| status.as_str()),
        "compliance_status": app.compliance_status().map(|status| status.as_str()),
        "resiliency_score": app.resiliency_score(),
        "assessment_schedule": app.assessment_schedule().map(|schedule| schedule.as_str()),
        "drift_status": app.drift_status().map(|status| status.as_str()),
        "last_app_compliance_evaluation_time": format_date(app.last_app_compliance_evaluation_time()),
        "rto_in_secs": app.rto_in_secs(),
        "rpo_in_secs": app.rpo_in_secs(),
        "aws_application_arn": app.aws_application_arn(),
        "policy_arn": detail.and_then(|app| app.policy_arn()),
        "event_subscription_count": detail.map(|app| app.event_subscriptions().len()).unwrap_or(0),
        "tag_count": detail
            .and_then(|app| app.tags())
            .map(|tags| tags.len())
            .unwrap_or(0),
    })
}

fn policy_to_value(policy: &ResiliencyPolicy) -> Value {
    let failure_policies: Vec<Value> = policy
        .policy()
        .map(|policies| {
            policies
                .iter()
                .map(|(disruption, failure_policy)| {
                    failure_policy_to_value(disruption.as_str(), failure_policy)
                })
                .collect()
        })
        .unwrap_or_default();

    json!({
        "policy_arn": policy.policy_arn(),
        "policy_name": policy.policy_name(),
        "policy_description": policy.policy_description(),
        "data_location_constraint": policy.data_location_constraint().map(|v| v.as_str()),
        "tier": policy.tier().map(|tier| tier.as_str()),
        "estimated_cost_tier": policy.estimated_cost_tier().map(|tier| tier.as_str()),
        "creation_time": format_date(policy.creation_time()),
        "tag_count": policy.tags().map(|tags| tags.len()).unwrap_or(0),
        "policy": failure_policies,
    })
}

fn failure_policy_to_value(disruption_type: &str, policy: &FailurePolicy) -> Value {
    json!({
        "disruption_type": disruption_type,
        "rto_in_secs": policy.rto_in_secs(),
        "rpo_in_secs": policy.rpo_in_secs(),
    })
}

fn assessment_to_value(assessment: &AppAssessmentSummary) -> Value {
    json!({
        "assessment_arn": assessment.assessment_arn(),
        "assessment_name": assessment.assessment_name(),
        "app_arn": assessment.app_arn(),
        "app_version": assessment.app_version(),
        "version_name": assessment.version_name(),
        "assessment_status": assessment.assessment_status().as_str(),
        "invoker": assessment.invoker().map(|invoker| invoker.as_str()),
        "compliance_status": assessment.compliance_status().map(|status| status.as_str()),
        "drift_status": assessment.drift_status().map(|status| status.as_str()),
        "resiliency_score": assessment.resiliency_score(),
        "cost": assessment.cost().map(cost_to_value),
        "message": assessment.message(),
        "start_time": format_date(assessment.start_time()),
        "end_time": format_date(assessment.end_time()),
    })
}

fn component_compliance_to_value(component: &AppComponentCompliance) -> Value {
    let compliance: Vec<Value> = component
        .compliance()
        .map(|items| {
            items
                .iter()
                .map(|(disruption, compliance)| {
                    disruption_compliance_to_value(disruption.as_str(), compliance)
                })
                .collect()
        })
        .unwrap_or_default();

    json!({
        "app_component_name": component.app_component_name(),
        "status": component.status().map(|status| status.as_str()),
        "message": component.message(),
        "cost": component.cost().map(cost_to_value),
        "resiliency_score": component.resiliency_score().map(|score| score.score()),
        "compliance": compliance,
    })
}

fn disruption_compliance_to_value(
    disruption_type: &str,
    compliance: &DisruptionCompliance,
) -> Value {
    json!({
        "disruption_type": disruption_type,
        "compliance_status": compliance.compliance_status().as_str(),
        "current_rto_in_secs": compliance.current_rto_in_secs(),
        "achievable_rto_in_secs": compliance.achievable_rto_in_secs(),
        "current_rpo_in_secs": compliance.current_rpo_in_secs(),
        "achievable_rpo_in_secs": compliance.achievable_rpo_in_secs(),
        "message": compliance.message(),
    })
}

fn cost_to_value(cost: &Cost) -> Value {
    json!({
        "amount": cost.amount(),
        "currency": cost.currency(),
        "frequency": cost.frequency().as_str(),
    })
}

fn app_compliance_is_noncompliant(
    status: &aws_sdk_resiliencehub::types::AppComplianceStatusType,
) -> bool {
    !matches!(status.as_str(), "PolicyMet" | "NotApplicable")
}

fn compliance_is_noncompliant(status: &aws_sdk_resiliencehub::types::ComplianceStatus) -> bool {
    !matches!(status.as_str(), "PolicyMet" | "NotApplicable")
}

fn component_is_noncompliant(component: &AppComponentCompliance) -> bool {
    if component
        .status()
        .map(compliance_is_noncompliant)
        .unwrap_or(false)
    {
        return true;
    }

    component
        .compliance()
        .map(|items| {
            items
                .values()
                .any(|compliance| compliance_is_noncompliant(compliance.compliance_status()))
        })
        .unwrap_or(false)
}

fn fallback_account_arn(aws_account_dto: &AwsAccountDto) -> String {
    format!(
        "arn:aws:resiliencehub:{}:{}:account/{}",
        aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
    )
}

fn format_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.and_then(|date| date.fmt(Format::DateTime).ok())
}
