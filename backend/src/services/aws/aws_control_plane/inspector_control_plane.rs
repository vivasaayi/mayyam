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
use aws_sdk_inspector2::types::{
    AccountState, CoveredResource, FailedAccount, Finding, ResourceState, State,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct InspectorControlPlane {
    aws_service: Arc<AwsService>,
}

impl InspectorControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_account_coverage(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Inspector account coverage for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_inspector_client(aws_account_dto)
            .await?;
        let account_status = client
            .batch_get_account_status()
            .account_ids(aws_account_dto.account_id.clone())
            .send()
            .await
            .map_err(|e| {
                error!("Failed to get Inspector account status: {}", e);
                AppError::ExternalService(format!("Failed to get Inspector account status: {}", e))
            })?;

        let account_state = account_status
            .accounts()
            .iter()
            .find(|account| account.account_id() == aws_account_dto.account_id);
        let failed_account = account_status
            .failed_accounts()
            .iter()
            .find(|account| account.account_id() == aws_account_dto.account_id);

        let arn = fallback_account_arn(aws_account_dto);
        let scan_state_summary =
            resource_state_to_json(account_state.and_then(|account| account.resource_state()));
        let coverage_summary = collect_coverage_summary(&client).await;
        let finding_summary = collect_finding_summary(&client).await;
        let tags = list_tags(&client, &arn).await;

        let mut resource_data = serde_json::Map::new();
        resource_data.insert(
            "account_status".to_string(),
            json!(account_status_value(account_state, failed_account)),
        );
        resource_data.insert(
            "account_error_code".to_string(),
            json!(account_error_code(account_state, failed_account)),
        );
        resource_data.insert(
            "account_error_message".to_string(),
            json!(account_error_message(account_state, failed_account)),
        );
        resource_data.insert("scan_state_summary".to_string(), scan_state_summary);
        resource_data.insert("coverage_summary".to_string(), coverage_summary.clone());
        resource_data.insert(
            "coverage_total_count".to_string(),
            json!(coverage_summary
                .get("total_count")
                .and_then(|value| value.as_u64())
                .unwrap_or_else(|| {
                    coverage_summary
                        .get("sample_count")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0)
                })),
        );
        resource_data.insert(
            "inactive_coverage_count".to_string(),
            json!(coverage_summary
                .get("inactive_coverage_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)),
        );
        resource_data.insert("finding_summary".to_string(), finding_summary.clone());
        resource_data.insert(
            "high_or_critical_active_findings".to_string(),
            json!(finding_summary
                .get("high_or_critical_active_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)),
        );
        resource_data.insert(
            "exploit_available_findings".to_string(),
            json!(finding_summary
                .get("exploit_available_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)),
        );
        resource_data.insert(
            "fix_available_findings".to_string(),
            json!(finding_summary
                .get("fix_available_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)),
        );

        let dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::InspectorAccountCoverage.to_string(),
            resource_id: format!(
                "inspector:{}:{}",
                aws_account_dto.default_region, aws_account_dto.account_id
            ),
            arn,
            name: Some(format!("Inspector {}", aws_account_dto.default_region)),
            tags,
            resource_data: Value::Object(resource_data),
        };

        debug!(
            "Successfully synced Inspector account coverage for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        Ok(vec![dto.into()])
    }
}

async fn collect_coverage_summary(client: &aws_sdk_inspector2::Client) -> Value {
    let (total_count, statistics_available) = match client.list_coverage_statistics().send().await {
        Ok(response) => (response.total_counts().max(0) as u64, true),
        Err(e) => {
            debug!("Failed to collect Inspector coverage statistics: {}", e);
            (0, false)
        }
    };

    let response = match client.list_coverage().max_results(100).send().await {
        Ok(response) => response,
        Err(e) => {
            debug!("Failed to collect Inspector coverage sample: {}", e);
            return json!({
                "total_count": total_count,
                "statistics_available": statistics_available,
                "sample_count": 0,
                "has_more": false,
                "inactive_coverage_count": 0,
                "sample_resources": [],
            });
        }
    };

    let mut inactive_coverage_count = 0usize;
    let mut sample_resources = Vec::new();

    for resource in response.covered_resources() {
        let scan_status_code = resource
            .scan_status()
            .map(|status| status.status_code().as_str());
        if scan_status_code != Some("ACTIVE") {
            inactive_coverage_count += 1;
        }

        if sample_resources.len() < 20 {
            sample_resources.push(covered_resource_to_json(resource));
        }
    }

    json!({
        "total_count": if total_count > 0 { total_count } else { response.covered_resources().len() as u64 },
        "statistics_available": statistics_available,
        "sample_count": response.covered_resources().len(),
        "has_more": response.next_token().is_some(),
        "inactive_coverage_count": inactive_coverage_count,
        "sample_resources": sample_resources,
    })
}

async fn collect_finding_summary(client: &aws_sdk_inspector2::Client) -> Value {
    let response = match client.list_findings().max_results(100).send().await {
        Ok(response) => response,
        Err(e) => {
            debug!("Failed to collect Inspector finding summary: {}", e);
            return json!({
                "sample_count": 0,
                "has_more": false,
                "high_or_critical_active_count": 0,
                "exploit_available_count": 0,
                "fix_available_count": 0,
                "active_count": 0,
                "sample_findings": [],
            });
        }
    };

    let mut active_count = 0usize;
    let mut high_or_critical_active_count = 0usize;
    let mut exploit_available_count = 0usize;
    let mut fix_available_count = 0usize;
    let mut sample_findings = Vec::new();

    for finding in response.findings() {
        let active = finding.status().as_str() == "ACTIVE";
        if active {
            active_count += 1;
            if matches!(finding.severity().as_str(), "HIGH" | "CRITICAL") {
                high_or_critical_active_count += 1;
            }
            if finding.exploit_available().map(|value| value.as_str()) == Some("YES") {
                exploit_available_count += 1;
            }
            if matches!(
                finding.fix_available().map(|value| value.as_str()),
                Some("YES" | "PARTIAL")
            ) {
                fix_available_count += 1;
            }
        }

        if sample_findings.len() < 10 {
            sample_findings.push(finding_to_json(finding));
        }
    }

    json!({
        "sample_count": response.findings().len(),
        "has_more": response.next_token().is_some(),
        "active_count": active_count,
        "high_or_critical_active_count": high_or_critical_active_count,
        "exploit_available_count": exploit_available_count,
        "fix_available_count": fix_available_count,
        "sample_findings": sample_findings,
    })
}

async fn list_tags(client: &aws_sdk_inspector2::Client, arn: &str) -> Value {
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
            debug!("Failed to list Inspector tags for {}: {}", arn, e);
        }
    }
    Value::Object(tags_map)
}

fn covered_resource_to_json(resource: &CoveredResource) -> Value {
    json!({
        "resource_type": resource.resource_type().as_str(),
        "resource_id": resource.resource_id(),
        "account_id": resource.account_id(),
        "scan_type": resource.scan_type().as_str(),
        "scan_status_code": resource
            .scan_status()
            .map(|status| status.status_code().as_str()),
        "scan_status_reason": resource
            .scan_status()
            .map(|status| status.reason().as_str()),
        "last_scanned_at": resource.last_scanned_at().map(|value| value.to_string()),
        "scan_mode": resource.scan_mode().map(|value| value.as_str()),
    })
}

fn finding_to_json(finding: &Finding) -> Value {
    json!({
        "finding_arn": finding.finding_arn(),
        "title": finding.title(),
        "type": finding.r#type().as_str(),
        "severity": finding.severity().as_str(),
        "status": finding.status().as_str(),
        "inspector_score": finding.inspector_score(),
        "fix_available": finding.fix_available().map(|value| value.as_str()),
        "exploit_available": finding.exploit_available().map(|value| value.as_str()),
        "updated_at": finding.updated_at().map(|value| value.to_string()),
        "resources": finding
            .resources()
            .iter()
            .take(5)
            .map(|resource| {
                json!({
                    "type": resource.r#type().as_str(),
                    "id": resource.id(),
                    "region": resource.region(),
                    "tags": resource.tags(),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn resource_state_to_json(resource_state: Option<&ResourceState>) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(resource_state) = resource_state {
        insert_state(&mut map, "ec2", resource_state.ec2());
        insert_state(&mut map, "ecr", resource_state.ecr());
        insert_state(&mut map, "lambda", resource_state.lambda());
        insert_state(&mut map, "lambda_code", resource_state.lambda_code());
        insert_state(
            &mut map,
            "code_repository",
            resource_state.code_repository(),
        );
    }
    Value::Object(map)
}

fn insert_state(map: &mut serde_json::Map<String, Value>, key: &str, state: Option<&State>) {
    if let Some(state) = state {
        map.insert(
            key.to_string(),
            json!({
                "status": state.status().as_str(),
                "error_code": state.error_code().as_str(),
                "error_message": state.error_message(),
            }),
        );
    }
}

fn account_status_value(
    account_state: Option<&AccountState>,
    failed_account: Option<&FailedAccount>,
) -> Option<String> {
    account_state
        .and_then(|account| account.state())
        .map(|state| state.status().as_str().to_string())
        .or_else(|| {
            failed_account
                .and_then(|account| account.status())
                .map(|status| status.as_str().to_string())
        })
}

fn account_error_code(
    account_state: Option<&AccountState>,
    failed_account: Option<&FailedAccount>,
) -> Option<String> {
    account_state
        .and_then(|account| account.state())
        .map(|state| state.error_code().as_str().to_string())
        .or_else(|| failed_account.map(|account| account.error_code().as_str().to_string()))
}

fn account_error_message(
    account_state: Option<&AccountState>,
    failed_account: Option<&FailedAccount>,
) -> Option<String> {
    account_state
        .and_then(|account| account.state())
        .map(|state| state.error_message().to_string())
        .or_else(|| failed_account.map(|account| account.error_message().to_string()))
}

fn fallback_account_arn(aws_account_dto: &AwsAccountDto) -> String {
    format!(
        "arn:aws:inspector2:{}:{}:account/{}",
        aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
    )
}
