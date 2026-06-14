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
use aws_sdk_macie2::operation::get_automated_discovery_configuration::GetAutomatedDiscoveryConfigurationOutput;
use aws_sdk_macie2::operation::get_bucket_statistics::GetBucketStatisticsOutput;
use aws_sdk_macie2::types::{
    BucketCountByEffectivePermission, BucketCountByEncryptionType, BucketCountBySharedAccessType,
    BucketCountPolicyAllowsUnencryptedObjectUploads, BucketStatisticsBySensitivity, Finding,
    JobSummary, SensitivityAggregations,
};
use aws_smithy_types::date_time::Format;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct MacieControlPlane {
    aws_service: Arc<AwsService>,
}

impl MacieControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_account(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Macie account inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_macie_client(aws_account_dto)
            .await?;
        let session = client.get_macie_session().send().await.map_err(|e| {
            error!("Failed to get Macie session: {}", e);
            AppError::ExternalService(format!("Failed to get Macie session: {}", e))
        })?;

        let arn = fallback_account_arn(aws_account_dto);
        let automated_discovery = collect_automated_discovery(&client).await;
        let bucket_statistics = collect_bucket_statistics(&client).await;
        let classification_jobs = collect_classification_jobs(&client).await;
        let finding_summary = collect_finding_summary(&client).await;
        let tags = list_tags(&client, &arn).await;

        let classification_job_count = classification_jobs.len();
        let active_classification_job_count = classification_jobs
            .iter()
            .filter(|job| {
                matches!(
                    job.get("job_status").and_then(|v| v.as_str()),
                    Some("RUNNING" | "IDLE")
                )
            })
            .count();

        let mut resource_data = serde_json::Map::new();
        resource_data.insert(
            "status".to_string(),
            json!(session.status().map(|value| value.as_str())),
        );
        resource_data.insert(
            "finding_publishing_frequency".to_string(),
            json!(session
                .finding_publishing_frequency()
                .map(|value| value.as_str())),
        );
        resource_data.insert("service_role".to_string(), json!(session.service_role()));
        resource_data.insert(
            "created_at".to_string(),
            json!(fmt_date(session.created_at())),
        );
        resource_data.insert(
            "updated_at".to_string(),
            json!(fmt_date(session.updated_at())),
        );
        resource_data.insert(
            "automated_discovery_status".to_string(),
            automated_discovery
                .get("status")
                .cloned()
                .unwrap_or(Value::Null),
        );
        resource_data.insert("automated_discovery".to_string(), automated_discovery);
        resource_data.insert("bucket_statistics".to_string(), bucket_statistics.clone());
        resource_data.insert(
            "bucket_count".to_string(),
            json!(value_count(&bucket_statistics, "bucket_count")),
        );
        resource_data.insert(
            "not_classified_bucket_count".to_string(),
            json!(value_count(
                &bucket_statistics,
                "not_classified_bucket_count"
            )),
        );
        resource_data.insert(
            "classification_error_bucket_count".to_string(),
            json!(value_count(
                &bucket_statistics,
                "classification_error_bucket_count"
            )),
        );
        resource_data.insert(
            "sensitive_bucket_count".to_string(),
            json!(value_count(&bucket_statistics, "sensitive_bucket_count")),
        );
        resource_data.insert(
            "public_bucket_count".to_string(),
            json!(value_count(&bucket_statistics, "public_bucket_count")),
        );
        resource_data.insert(
            "unknown_permission_bucket_count".to_string(),
            json!(value_count(
                &bucket_statistics,
                "unknown_permission_bucket_count"
            )),
        );
        resource_data.insert(
            "unknown_encryption_bucket_count".to_string(),
            json!(value_count(
                &bucket_statistics,
                "unknown_encryption_bucket_count"
            )),
        );
        resource_data.insert(
            "unknown_shared_access_bucket_count".to_string(),
            json!(value_count(
                &bucket_statistics,
                "unknown_shared_access_bucket_count"
            )),
        );
        resource_data.insert(
            "classification_job_count".to_string(),
            json!(classification_job_count),
        );
        resource_data.insert(
            "active_classification_job_count".to_string(),
            json!(active_classification_job_count),
        );
        resource_data.insert(
            "classification_jobs".to_string(),
            json!(classification_jobs),
        );
        resource_data.insert("finding_summary".to_string(), finding_summary.clone());
        resource_data.insert(
            "policy_finding_count".to_string(),
            json!(value_count(&finding_summary, "policy_finding_count")),
        );
        resource_data.insert(
            "sensitive_data_finding_count".to_string(),
            json!(value_count(
                &finding_summary,
                "sensitive_data_finding_count"
            )),
        );

        let dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::MacieAccount.to_string(),
            resource_id: format!(
                "macie:{}:{}",
                aws_account_dto.default_region, aws_account_dto.account_id
            ),
            arn,
            name: Some(format!("Macie {}", aws_account_dto.default_region)),
            tags,
            resource_data: Value::Object(resource_data),
        };

        debug!(
            "Successfully synced Macie account inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        Ok(vec![dto.into()])
    }
}

async fn collect_automated_discovery(client: &aws_sdk_macie2::Client) -> Value {
    match client.get_automated_discovery_configuration().send().await {
        Ok(response) => automated_discovery_to_json(&response),
        Err(e) => {
            debug!(
                "Failed to get Macie automated discovery configuration: {}",
                e
            );
            json!({
                "available": false,
                "status": Value::Null,
            })
        }
    }
}

async fn collect_bucket_statistics(client: &aws_sdk_macie2::Client) -> Value {
    match client.get_bucket_statistics().send().await {
        Ok(response) => bucket_statistics_to_json(&response),
        Err(e) => {
            debug!("Failed to get Macie bucket statistics: {}", e);
            json!({
                "available": false,
                "bucket_count": 0,
                "not_classified_bucket_count": 0,
                "classification_error_bucket_count": 0,
                "sensitive_bucket_count": 0,
                "public_bucket_count": 0,
                "unknown_permission_bucket_count": 0,
                "unknown_encryption_bucket_count": 0,
                "unknown_shared_access_bucket_count": 0,
            })
        }
    }
}

async fn collect_classification_jobs(client: &aws_sdk_macie2::Client) -> Vec<Value> {
    let mut jobs = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_classification_jobs().max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!("Failed to list Macie classification jobs: {}", e);
                return jobs;
            }
        };

        jobs.extend(response.items().iter().map(job_to_json));
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    jobs
}

async fn collect_finding_summary(client: &aws_sdk_macie2::Client) -> Value {
    let response = match client.list_findings().max_results(50).send().await {
        Ok(response) => response,
        Err(e) => {
            debug!("Failed to list Macie findings: {}", e);
            return json!({
                "sample_count": 0,
                "has_more": false,
                "policy_finding_count": 0,
                "sensitive_data_finding_count": 0,
                "high_severity_finding_count": 0,
                "sample_findings": [],
            });
        }
    };

    let finding_ids: Vec<String> = response.finding_ids().iter().cloned().collect();
    if finding_ids.is_empty() {
        return json!({
            "sample_count": 0,
            "has_more": response.next_token().is_some(),
            "policy_finding_count": 0,
            "sensitive_data_finding_count": 0,
            "high_severity_finding_count": 0,
            "sample_findings": [],
        });
    }

    let findings = match client
        .get_findings()
        .set_finding_ids(Some(finding_ids.clone()))
        .send()
        .await
    {
        Ok(response) => response.findings().to_vec(),
        Err(e) => {
            debug!("Failed to get Macie finding details: {}", e);
            Vec::new()
        }
    };

    let mut policy_finding_count = 0usize;
    let mut sensitive_data_finding_count = 0usize;
    let mut high_severity_finding_count = 0usize;
    let mut sample_findings = Vec::new();

    for finding in &findings {
        if finding.archived().unwrap_or(false) {
            continue;
        }

        match finding.category().map(|value| value.as_str()) {
            Some("POLICY") => policy_finding_count += 1,
            Some("CLASSIFICATION") => sensitive_data_finding_count += 1,
            _ => {}
        }
        if finding
            .severity()
            .and_then(|severity| severity.score())
            .unwrap_or(0)
            >= 3
        {
            high_severity_finding_count += 1;
        }
        if sample_findings.len() < 10 {
            sample_findings.push(finding_to_json(finding));
        }
    }

    json!({
        "sample_count": findings.len(),
        "listed_finding_count": finding_ids.len(),
        "has_more": response.next_token().is_some(),
        "policy_finding_count": policy_finding_count,
        "sensitive_data_finding_count": sensitive_data_finding_count,
        "high_severity_finding_count": high_severity_finding_count,
        "sample_findings": sample_findings,
    })
}

async fn list_tags(client: &aws_sdk_macie2::Client, arn: &str) -> Value {
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
            debug!("Failed to list Macie tags for {}: {}", arn, e);
        }
    }
    Value::Object(tags_map)
}

fn automated_discovery_to_json(response: &GetAutomatedDiscoveryConfigurationOutput) -> Value {
    json!({
        "available": true,
        "status": response.status().map(|value| value.as_str()),
        "auto_enable_organization_members": response
            .auto_enable_organization_members()
            .map(|value| value.as_str()),
        "classification_scope_id": response.classification_scope_id(),
        "sensitivity_inspection_template_id": response.sensitivity_inspection_template_id(),
        "disabled_at": fmt_date(response.disabled_at()),
        "first_enabled_at": fmt_date(response.first_enabled_at()),
        "last_updated_at": fmt_date(response.last_updated_at()),
    })
}

fn bucket_statistics_to_json(response: &GetBucketStatisticsOutput) -> Value {
    let sensitivity = response.bucket_statistics_by_sensitivity();
    let effective_permission = response.bucket_count_by_effective_permission();
    let encryption_type = response.bucket_count_by_encryption_type();
    let object_encryption_requirement = response.bucket_count_by_object_encryption_requirement();
    let shared_access_type = response.bucket_count_by_shared_access_type();

    let not_classified_bucket_count = sensitivity
        .and_then(|stats| stats.not_classified())
        .and_then(|stats| stats.total_count())
        .unwrap_or(0);
    let classification_error_bucket_count = sensitivity
        .and_then(|stats| stats.classification_error())
        .and_then(|stats| stats.total_count())
        .unwrap_or(0);
    let sensitive_bucket_count = sensitivity
        .and_then(|stats| stats.sensitive())
        .and_then(|stats| stats.total_count())
        .unwrap_or(0);
    let public_bucket_count = effective_permission
        .and_then(|stats| stats.publicly_accessible())
        .unwrap_or(0)
        .max(public_sensitivity_count(sensitivity));
    let unknown_encryption_bucket_count = encryption_type
        .and_then(|stats| stats.unknown())
        .unwrap_or(0)
        + object_encryption_requirement
            .and_then(|stats| stats.unknown())
            .unwrap_or(0);

    json!({
        "available": true,
        "bucket_count": count(response.bucket_count()),
        "object_count": count(response.object_count()),
        "size_in_bytes": count(response.size_in_bytes()),
        "classifiable_object_count": count(response.classifiable_object_count()),
        "classifiable_size_in_bytes": count(response.classifiable_size_in_bytes()),
        "last_updated": fmt_date(response.last_updated()),
        "effective_permission": effective_permission_to_json(effective_permission),
        "encryption_type": encryption_type_to_json(encryption_type),
        "object_encryption_requirement": object_encryption_requirement_to_json(
            object_encryption_requirement,
        ),
        "shared_access_type": shared_access_type_to_json(shared_access_type),
        "sensitivity": sensitivity_to_json(sensitivity),
        "not_classified_bucket_count": count(Some(not_classified_bucket_count)),
        "classification_error_bucket_count": count(Some(classification_error_bucket_count)),
        "sensitive_bucket_count": count(Some(sensitive_bucket_count)),
        "public_bucket_count": count(Some(public_bucket_count)),
        "unknown_permission_bucket_count": count(
            effective_permission.and_then(|stats| stats.unknown()),
        ),
        "unknown_encryption_bucket_count": count(Some(unknown_encryption_bucket_count)),
        "unknown_shared_access_bucket_count": count(
            shared_access_type.and_then(|stats| stats.unknown()),
        ),
    })
}

fn effective_permission_to_json(stats: Option<&BucketCountByEffectivePermission>) -> Value {
    json!({
        "publicly_accessible": count(stats.and_then(|stats| stats.publicly_accessible())),
        "publicly_readable": count(stats.and_then(|stats| stats.publicly_readable())),
        "publicly_writable": count(stats.and_then(|stats| stats.publicly_writable())),
        "unknown": count(stats.and_then(|stats| stats.unknown())),
    })
}

fn encryption_type_to_json(stats: Option<&BucketCountByEncryptionType>) -> Value {
    json!({
        "kms_managed": count(stats.and_then(|stats| stats.kms_managed())),
        "s3_managed": count(stats.and_then(|stats| stats.s3_managed())),
        "unencrypted": count(stats.and_then(|stats| stats.unencrypted())),
        "unknown": count(stats.and_then(|stats| stats.unknown())),
    })
}

fn object_encryption_requirement_to_json(
    stats: Option<&BucketCountPolicyAllowsUnencryptedObjectUploads>,
) -> Value {
    json!({
        "allows_unencrypted_object_uploads": count(
            stats.and_then(|stats| stats.allows_unencrypted_object_uploads()),
        ),
        "denies_unencrypted_object_uploads": count(
            stats.and_then(|stats| stats.denies_unencrypted_object_uploads()),
        ),
        "unknown": count(stats.and_then(|stats| stats.unknown())),
    })
}

fn shared_access_type_to_json(stats: Option<&BucketCountBySharedAccessType>) -> Value {
    json!({
        "external": count(stats.and_then(|stats| stats.external())),
        "internal": count(stats.and_then(|stats| stats.internal())),
        "not_shared": count(stats.and_then(|stats| stats.not_shared())),
        "unknown": count(stats.and_then(|stats| stats.unknown())),
    })
}

fn sensitivity_to_json(stats: Option<&BucketStatisticsBySensitivity>) -> Value {
    json!({
        "classification_error": sensitivity_aggregation_to_json(
            stats.and_then(|stats| stats.classification_error()),
        ),
        "not_classified": sensitivity_aggregation_to_json(
            stats.and_then(|stats| stats.not_classified()),
        ),
        "not_sensitive": sensitivity_aggregation_to_json(
            stats.and_then(|stats| stats.not_sensitive()),
        ),
        "sensitive": sensitivity_aggregation_to_json(
            stats.and_then(|stats| stats.sensitive()),
        ),
    })
}

fn sensitivity_aggregation_to_json(stats: Option<&SensitivityAggregations>) -> Value {
    json!({
        "total_count": count(stats.and_then(|stats| stats.total_count())),
        "publicly_accessible_count": count(
            stats.and_then(|stats| stats.publicly_accessible_count()),
        ),
        "classifiable_size_in_bytes": count(
            stats.and_then(|stats| stats.classifiable_size_in_bytes()),
        ),
        "total_size_in_bytes": count(stats.and_then(|stats| stats.total_size_in_bytes())),
    })
}

fn public_sensitivity_count(stats: Option<&BucketStatisticsBySensitivity>) -> i64 {
    [
        stats.and_then(|stats| stats.classification_error()),
        stats.and_then(|stats| stats.not_classified()),
        stats.and_then(|stats| stats.not_sensitive()),
        stats.and_then(|stats| stats.sensitive()),
    ]
    .into_iter()
    .filter_map(|aggregation| aggregation.and_then(|value| value.publicly_accessible_count()))
    .sum()
}

fn job_to_json(job: &JobSummary) -> Value {
    json!({
        "job_id": job.job_id(),
        "name": job.name(),
        "job_status": job.job_status().map(|value| value.as_str()),
        "job_type": job.job_type().map(|value| value.as_str()),
        "created_at": fmt_date(job.created_at()),
        "last_run_error_status": job
            .last_run_error_status()
            .and_then(|status| status.code())
            .map(|value| value.as_str()),
    })
}

fn finding_to_json(finding: &Finding) -> Value {
    json!({
        "id": finding.id(),
        "title": finding.title(),
        "category": finding.category().map(|value| value.as_str()),
        "type": finding.r#type().map(|value| value.as_str()),
        "archived": finding.archived().unwrap_or(false),
        "severity": finding
            .severity()
            .and_then(|severity| severity.description())
            .map(|value| value.as_str()),
        "severity_score": finding.severity().and_then(|severity| severity.score()),
        "count": finding.count(),
        "created_at": fmt_date(finding.created_at()),
        "updated_at": fmt_date(finding.updated_at()),
    })
}

fn count(value: Option<i64>) -> u64 {
    value.unwrap_or(0).max(0) as u64
}

fn value_count(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(|value| value.as_u64()).unwrap_or(0)
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.and_then(|value| value.fmt(Format::DateTime).ok())
}

fn fallback_account_arn(aws_account_dto: &AwsAccountDto) -> String {
    format!(
        "arn:aws:macie2:{}:{}:configuration",
        aws_account_dto.default_region, aws_account_dto.account_id
    )
}
