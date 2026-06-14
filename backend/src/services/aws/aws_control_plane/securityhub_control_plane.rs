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
use aws_sdk_securityhub::types::{AwsSecurityFinding, StandardsSubscription};
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct SecurityHubControlPlane {
    aws_service: Arc<AwsService>,
}

impl SecurityHubControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_hubs(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Security Hub hub for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_securityhub_client(aws_account_dto)
            .await?;
        let hub = client.describe_hub().send().await.map_err(|e| {
            error!("Failed to describe Security Hub hub: {}", e);
            AppError::ExternalService(format!("Failed to describe Security Hub hub: {}", e))
        })?;

        let arn = hub
            .hub_arn()
            .map(String::from)
            .unwrap_or_else(|| fallback_hub_arn(aws_account_dto));
        let standards = collect_enabled_standards(&client).await;
        let product_subscriptions = collect_product_subscriptions(&client).await;
        let finding_summary = collect_finding_summary(&client).await;
        let tags = list_tags(&client, &arn).await;

        let standards_ready_count = standards
            .iter()
            .filter(|standard| standard.get("status").and_then(|v| v.as_str()) == Some("READY"))
            .count();
        let standards_not_ready_count = standards.len().saturating_sub(standards_ready_count);

        let mut resource_data = serde_json::Map::new();
        resource_data.insert("hub_arn".to_string(), json!(arn));
        resource_data.insert("subscribed_at".to_string(), json!(hub.subscribed_at()));
        resource_data.insert(
            "auto_enable_controls".to_string(),
            json!(hub.auto_enable_controls().unwrap_or(false)),
        );
        resource_data.insert(
            "control_finding_generator".to_string(),
            json!(hub.control_finding_generator().map(|value| value.as_str())),
        );
        resource_data.insert("standards".to_string(), json!(standards));
        resource_data.insert(
            "enabled_standards_count".to_string(),
            json!(standards_ready_count + standards_not_ready_count),
        );
        resource_data.insert(
            "standards_ready_count".to_string(),
            json!(standards_ready_count),
        );
        resource_data.insert(
            "standards_not_ready_count".to_string(),
            json!(standards_not_ready_count),
        );
        resource_data.insert(
            "product_subscriptions".to_string(),
            json!(product_subscriptions),
        );
        resource_data.insert(
            "product_subscription_count".to_string(),
            json!(product_subscriptions.len()),
        );
        resource_data.insert("finding_summary".to_string(), finding_summary.clone());
        if let Some(count) = finding_summary
            .get("high_or_critical_active_unresolved_count")
            .and_then(|v| v.as_u64())
        {
            resource_data.insert(
                "high_or_critical_active_unresolved_findings".to_string(),
                json!(count),
            );
        }
        if let Some(count) = finding_summary
            .get("failed_control_finding_count")
            .and_then(|v| v.as_u64())
        {
            resource_data.insert("failed_control_finding_count".to_string(), json!(count));
        }

        let dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::SecurityHubHub.to_string(),
            resource_id: format!(
                "securityhub:{}:{}",
                aws_account_dto.default_region, aws_account_dto.account_id
            ),
            arn,
            name: Some(format!("Security Hub {}", aws_account_dto.default_region)),
            tags,
            resource_data: serde_json::Value::Object(resource_data),
        };

        debug!(
            "Successfully synced Security Hub hub for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        Ok(vec![dto.into()])
    }
}

async fn collect_enabled_standards(client: &aws_sdk_securityhub::Client) -> Vec<serde_json::Value> {
    let mut standards: Vec<serde_json::Value> = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.get_enabled_standards().max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!("Failed to list Security Hub enabled standards: {}", e);
                return standards;
            }
        };

        standards.extend(
            response
                .standards_subscriptions()
                .iter()
                .map(standard_to_json),
        );
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    standards
}

async fn collect_product_subscriptions(client: &aws_sdk_securityhub::Client) -> Vec<String> {
    let mut subscriptions: Vec<String> = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_enabled_products_for_import().max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list Security Hub enabled product subscriptions: {}",
                    e
                );
                return subscriptions;
            }
        };

        subscriptions.extend(response.product_subscriptions().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    subscriptions
}

async fn collect_finding_summary(client: &aws_sdk_securityhub::Client) -> serde_json::Value {
    let response = match client.get_findings().max_results(100).send().await {
        Ok(response) => response,
        Err(e) => {
            debug!("Failed to collect Security Hub finding summary: {}", e);
            return json!({
                "sample_count": 0,
                "high_or_critical_active_unresolved_count": 0,
                "failed_control_finding_count": 0,
                "active_unresolved_count": 0,
                "sample_findings": [],
            });
        }
    };

    let mut high_or_critical_active_unresolved_count = 0usize;
    let mut failed_control_finding_count = 0usize;
    let mut active_unresolved_count = 0usize;
    let mut product_names: BTreeSet<String> = BTreeSet::new();
    let mut samples: Vec<serde_json::Value> = Vec::new();

    for finding in response.findings() {
        if let Some(product_name) = finding.product_name() {
            product_names.insert(product_name.to_string());
        }

        let severity = finding_severity(finding);
        let workflow = finding_workflow(finding);
        let record_state = finding.record_state().map(|value| value.as_str());
        let compliance = finding_compliance(finding);
        let active = record_state.unwrap_or("ACTIVE") == "ACTIVE";
        let unresolved = !matches!(workflow, Some("RESOLVED" | "SUPPRESSED"));

        if active && unresolved {
            active_unresolved_count += 1;
            if matches!(severity, Some("HIGH" | "CRITICAL")) {
                high_or_critical_active_unresolved_count += 1;
            }
        }
        if active && unresolved && compliance == Some("FAILED") {
            failed_control_finding_count += 1;
        }

        if samples.len() < 10 {
            samples.push(json!({
                "id": finding.id(),
                "title": finding.title(),
                "product_name": finding.product_name(),
                "generator_id": finding.generator_id(),
                "severity": severity,
                "workflow_status": workflow,
                "record_state": record_state,
                "compliance_status": compliance,
                "updated_at": finding.updated_at(),
            }));
        }
    }

    json!({
        "sample_count": response.findings().len(),
        "has_more": response.next_token().is_some(),
        "high_or_critical_active_unresolved_count": high_or_critical_active_unresolved_count,
        "failed_control_finding_count": failed_control_finding_count,
        "active_unresolved_count": active_unresolved_count,
        "imported_product_names": product_names.into_iter().collect::<Vec<_>>(),
        "sample_findings": samples,
    })
}

async fn list_tags(client: &aws_sdk_securityhub::Client, arn: &str) -> serde_json::Value {
    let mut tags_map = serde_json::Map::new();
    match client
        .list_tags_for_resource()
        .resource_arn(arn)
        .send()
        .await
    {
        Ok(response) => {
            if let Some(tags) = response.tags() {
                for (key, value) in tags {
                    tags_map.insert(key.to_string(), json!(value));
                }
            }
        }
        Err(e) => {
            debug!("Failed to list Security Hub tags for {}: {}", arn, e);
        }
    }
    serde_json::Value::Object(tags_map)
}

fn standard_to_json(subscription: &StandardsSubscription) -> serde_json::Value {
    json!({
        "standards_subscription_arn": subscription.standards_subscription_arn(),
        "standards_arn": subscription.standards_arn(),
        "status": subscription.standards_status().map(|value| value.as_str()),
        "controls_updatable": subscription
            .standards_controls_updatable()
            .map(|value| value.as_str()),
        "status_reason_code": subscription
            .standards_status_reason()
            .and_then(|reason| reason.status_reason_code())
            .map(|value| value.as_str()),
        "input": subscription.standards_input(),
    })
}

fn finding_severity(finding: &AwsSecurityFinding) -> Option<&str> {
    finding
        .severity()
        .and_then(|severity| severity.label())
        .map(|label| label.as_str())
}

fn finding_workflow(finding: &AwsSecurityFinding) -> Option<&str> {
    finding
        .workflow()
        .and_then(|workflow| workflow.status())
        .map(|status| status.as_str())
}

fn finding_compliance(finding: &AwsSecurityFinding) -> Option<&str> {
    finding
        .compliance()
        .and_then(|compliance| compliance.status())
        .map(|status| status.as_str())
}

fn fallback_hub_arn(aws_account_dto: &AwsAccountDto) -> String {
    format!(
        "arn:aws:securityhub:{}:{}:hub/default",
        aws_account_dto.default_region, aws_account_dto.account_id
    )
}
