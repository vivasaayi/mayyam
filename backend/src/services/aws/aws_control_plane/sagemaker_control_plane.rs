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
use aws_sdk_cloudcontrol::types::ResourceDescription;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

const SAGEMAKER_TYPES: &[SageMakerType] = &[
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::NotebookInstance",
        resource_kind: "notebook_instance",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::Endpoint",
        resource_kind: "endpoint",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::EndpointConfig",
        resource_kind: "endpoint_config",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::Model",
        resource_kind: "model",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::TrainingJob",
        resource_kind: "training_job",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::TransformJob",
        resource_kind: "transform_job",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::ProcessingJob",
        resource_kind: "processing_job",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::Pipeline",
        resource_kind: "pipeline",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::Domain",
        resource_kind: "domain",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::UserProfile",
        resource_kind: "user_profile",
    },
    SageMakerType {
        cloudcontrol_type: "AWS::SageMaker::FeatureGroup",
        resource_kind: "feature_group",
    },
];

pub struct SageMakerControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Copy, Clone, Debug)]
struct SageMakerType {
    cloudcontrol_type: &'static str,
    resource_kind: &'static str,
}

#[derive(Debug, Default)]
struct CollectionSummary {
    resources_by_kind: BTreeMap<String, usize>,
    collection_errors: Vec<Value>,
    collection_error_count: usize,
    resource_count: usize,
    untagged_resource_count: usize,
    running_notebook_count: usize,
    failed_notebook_count: usize,
    endpoint_capacity_instance_count: i64,
    unhealthy_endpoint_count: usize,
    endpoint_config_without_kms_count: usize,
    model_without_vpc_count: usize,
    model_network_isolation_disabled_count: usize,
    notebook_direct_internet_enabled_count: usize,
    notebook_root_access_enabled_count: usize,
    unencrypted_notebook_volume_count: usize,
    failed_job_count: usize,
    incomplete_job_count: usize,
    domain_without_kms_count: usize,
    domain_not_ready_count: usize,
}

impl SageMakerControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS SageMaker AI inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_cloudcontrol_client(aws_account_dto)
            .await?;
        let mut resources = Vec::new();
        let mut summary = CollectionSummary::default();

        for sagemaker_type in SAGEMAKER_TYPES {
            match list_cloudcontrol_resources(&client, sagemaker_type.cloudcontrol_type).await {
                Ok(descriptions) => {
                    for description in descriptions {
                        resources.push(resource_from_description(
                            aws_account_dto,
                            sync_id,
                            sagemaker_type,
                            &description,
                            &mut summary,
                        ));
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to list SageMaker AI resources for {} through Cloud Control: {}",
                        sagemaker_type.cloudcontrol_type, e
                    );
                    summary.collection_error_count += 1;
                    summary.collection_errors.push(json!({
                        "cloudcontrol_type": sagemaker_type.cloudcontrol_type,
                        "error": e,
                    }));
                }
            }
        }

        resources.insert(
            0,
            account_summary_resource(aws_account_dto, sync_id, &summary),
        );
        Ok(resources)
    }
}

async fn list_cloudcontrol_resources(
    client: &aws_sdk_cloudcontrol::Client,
    type_name: &str,
) -> Result<Vec<ResourceDescription>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_resources().type_name(type_name);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.resource_descriptions().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

fn resource_from_description(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    sagemaker_type: &SageMakerType,
    description: &ResourceDescription,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let identifier = description.identifier().unwrap_or("unknown").to_string();
    let properties = parse_properties(description.properties());
    let tags = tags_from_properties(&properties);
    let resource_name = resource_name(sagemaker_type.resource_kind, &identifier, &properties);

    summary.resource_count += 1;
    *summary
        .resources_by_kind
        .entry(sagemaker_type.resource_kind.to_string())
        .or_insert(0) += 1;
    if tags.as_object().map(|tags| tags.is_empty()).unwrap_or(true) {
        summary.untagged_resource_count += 1;
    }
    record_sagemaker_evidence_counts(sagemaker_type.resource_kind, &properties, summary);

    let mut resource_data = serde_json::Map::new();
    resource_data.insert(
        "resource_kind".to_string(),
        json!(sagemaker_type.resource_kind),
    );
    resource_data.insert(
        "cloudcontrol_type".to_string(),
        json!(sagemaker_type.cloudcontrol_type),
    );
    resource_data.insert("cloudcontrol_identifier".to_string(), json!(identifier));
    resource_data.insert("properties".to_string(), properties.clone());
    insert_optional_str(&mut resource_data, "status", status(&properties));
    insert_optional_str(
        &mut resource_data,
        "instance_type",
        first_str(&properties, &["InstanceType"]),
    );
    insert_optional_str(
        &mut resource_data,
        "endpoint_config_name",
        first_str(&properties, &["EndpointConfigName"]),
    );
    insert_optional_str(
        &mut resource_data,
        "role_arn",
        first_str(&properties, &["RoleArn", "ExecutionRoleArn"]),
    );
    insert_optional_str(
        &mut resource_data,
        "kms_key_id",
        first_str(
            &properties,
            &[
                "KmsKeyId",
                "VolumeKmsKeyId",
                "OutputKmsKeyId",
                "KmsKey",
                "S3KmsKeyId",
            ],
        ),
    );
    insert_optional_str(
        &mut resource_data,
        "direct_internet_access",
        first_str(&properties, &["DirectInternetAccess"]),
    );
    insert_optional_str(
        &mut resource_data,
        "root_access",
        first_str(&properties, &["RootAccess"]),
    );
    if let Some(volume_size) = number_or_string(&properties, "VolumeSizeInGB") {
        resource_data.insert("volume_size_gb".to_string(), volume_size);
    }
    if let Some(enabled) = bool_value(&properties, "EnableNetworkIsolation") {
        resource_data.insert("network_isolation_enabled".to_string(), json!(enabled));
    }
    resource_data.insert(
        "vpc_configured".to_string(),
        json!(vpc_configured(&properties)),
    );
    resource_data.insert(
        "production_variant_count".to_string(),
        json!(array_len(&properties, "ProductionVariants")),
    );
    resource_data.insert(
        "endpoint_capacity_instance_count".to_string(),
        json!(sum_array_i64(
            &properties,
            "ProductionVariants",
            "InitialInstanceCount"
        )),
    );
    resource_data.insert(
        "serverless_variant_count".to_string(),
        json!(count_array_object_with_key(
            &properties,
            "ProductionVariants",
            "ServerlessConfig"
        )),
    );
    resource_data.insert(
        "data_capture_enabled".to_string(),
        json!(data_capture_enabled(&properties)),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::SageMakerResource.to_string(),
        resource_id: format!("{}/{}", sagemaker_type.resource_kind, resource_name),
        arn: resource_arn(
            sagemaker_type.resource_kind,
            description.identifier().unwrap_or("unknown"),
            &properties,
            aws_account_dto,
        ),
        name: Some(resource_name),
        tags,
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn account_summary_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    summary: &CollectionSummary,
) -> AwsResourceModel {
    let mut resource_data = serde_json::Map::new();
    resource_data.insert("resource_kind".to_string(), json!("account_summary"));
    resource_data.insert("account_id".to_string(), json!(&aws_account_dto.account_id));
    resource_data.insert("resource_count".to_string(), json!(summary.resource_count));
    resource_data.insert(
        "collection_error_count".to_string(),
        json!(summary.collection_error_count),
    );
    resource_data.insert(
        "collection_errors".to_string(),
        Value::Array(summary.collection_errors.clone()),
    );
    resource_data.insert(
        "resources_by_kind".to_string(),
        json!(summary.resources_by_kind),
    );
    resource_data.insert(
        "untagged_resource_count".to_string(),
        json!(summary.untagged_resource_count),
    );
    resource_data.insert(
        "running_notebook_count".to_string(),
        json!(summary.running_notebook_count),
    );
    resource_data.insert(
        "failed_notebook_count".to_string(),
        json!(summary.failed_notebook_count),
    );
    resource_data.insert(
        "endpoint_capacity_instance_count".to_string(),
        json!(summary.endpoint_capacity_instance_count),
    );
    resource_data.insert(
        "unhealthy_endpoint_count".to_string(),
        json!(summary.unhealthy_endpoint_count),
    );
    resource_data.insert(
        "endpoint_config_without_kms_count".to_string(),
        json!(summary.endpoint_config_without_kms_count),
    );
    resource_data.insert(
        "model_without_vpc_count".to_string(),
        json!(summary.model_without_vpc_count),
    );
    resource_data.insert(
        "model_network_isolation_disabled_count".to_string(),
        json!(summary.model_network_isolation_disabled_count),
    );
    resource_data.insert(
        "notebook_direct_internet_enabled_count".to_string(),
        json!(summary.notebook_direct_internet_enabled_count),
    );
    resource_data.insert(
        "notebook_root_access_enabled_count".to_string(),
        json!(summary.notebook_root_access_enabled_count),
    );
    resource_data.insert(
        "unencrypted_notebook_volume_count".to_string(),
        json!(summary.unencrypted_notebook_volume_count),
    );
    resource_data.insert(
        "failed_job_count".to_string(),
        json!(summary.failed_job_count),
    );
    resource_data.insert(
        "incomplete_job_count".to_string(),
        json!(summary.incomplete_job_count),
    );
    resource_data.insert(
        "domain_without_kms_count".to_string(),
        json!(summary.domain_without_kms_count),
    );
    resource_data.insert(
        "domain_not_ready_count".to_string(),
        json!(summary.domain_not_ready_count),
    );
    resource_data.insert(
        "cloudcontrol_type_count".to_string(),
        json!(SAGEMAKER_TYPES.len()),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::SageMakerResource.to_string(),
        resource_id: format!("sagemaker:{}", aws_account_dto.account_id),
        arn: format!(
            "arn:aws:sagemaker:{}:{}:account/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
        ),
        name: Some("SageMaker AI".to_string()),
        tags: json!({}),
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn parse_properties(raw: Option<&str>) -> Value {
    raw.and_then(|properties| serde_json::from_str(properties).ok())
        .unwrap_or_else(|| json!({}))
}

fn tags_from_properties(properties: &Value) -> Value {
    let Some(tags) = properties.get("Tags").and_then(|v| v.as_array()) else {
        return json!({});
    };

    let mut tag_map = serde_json::Map::new();
    for tag in tags {
        let key = tag
            .get("Key")
            .or_else(|| tag.get("key"))
            .and_then(|v| v.as_str());
        let value = tag
            .get("Value")
            .or_else(|| tag.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if let Some(key) = key {
            tag_map.insert(key.to_string(), json!(value));
        }
    }

    Value::Object(tag_map)
}

fn record_sagemaker_evidence_counts(
    resource_kind: &str,
    properties: &Value,
    summary: &mut CollectionSummary,
) {
    match resource_kind {
        "notebook_instance" => {
            if status(properties)
                .map(is_running_notebook_status)
                .unwrap_or(false)
            {
                summary.running_notebook_count += 1;
            }
            if status(properties)
                .map(|status| status.eq_ignore_ascii_case("failed"))
                .unwrap_or(false)
            {
                summary.failed_notebook_count += 1;
            }
            if str_eq(properties, "DirectInternetAccess", "Enabled") {
                summary.notebook_direct_internet_enabled_count += 1;
            }
            if str_eq(properties, "RootAccess", "Enabled") {
                summary.notebook_root_access_enabled_count += 1;
            }
            if first_str(properties, &["KmsKeyId", "VolumeKmsKeyId"]).is_none() {
                summary.unencrypted_notebook_volume_count += 1;
            }
        }
        "endpoint" => {
            if status(properties)
                .map(is_unhealthy_endpoint_status)
                .unwrap_or(false)
            {
                summary.unhealthy_endpoint_count += 1;
            }
        }
        "endpoint_config" => {
            summary.endpoint_capacity_instance_count +=
                sum_array_i64(properties, "ProductionVariants", "InitialInstanceCount");
            if first_str(properties, &["KmsKeyId"]).is_none() {
                summary.endpoint_config_without_kms_count += 1;
            }
        }
        "model" => {
            if !vpc_configured(properties) {
                summary.model_without_vpc_count += 1;
            }
            if bool_value(properties, "EnableNetworkIsolation") != Some(true) {
                summary.model_network_isolation_disabled_count += 1;
            }
        }
        "training_job" | "transform_job" | "processing_job" => {
            if status(properties).map(is_failed_status).unwrap_or(false) {
                summary.failed_job_count += 1;
            } else if status(properties)
                .map(|status| !is_completed_job_status(status))
                .unwrap_or(false)
            {
                summary.incomplete_job_count += 1;
            }
        }
        "domain" => {
            if first_str(
                properties,
                &["KmsKeyId", "KmsKeyID", "HomeEfsFileSystemKmsKeyId"],
            )
            .is_none()
            {
                summary.domain_without_kms_count += 1;
            }
            if status(properties)
                .map(|status| !is_ready_status(status))
                .unwrap_or(false)
            {
                summary.domain_not_ready_count += 1;
            }
        }
        _ => {}
    }
}

fn resource_name(resource_kind: &str, identifier: &str, properties: &Value) -> String {
    let keys = match resource_kind {
        "notebook_instance" => &["NotebookInstanceName"][..],
        "endpoint" => &["EndpointName"][..],
        "endpoint_config" => &["EndpointConfigName"][..],
        "model" => &["ModelName"][..],
        "training_job" => &["TrainingJobName"][..],
        "transform_job" => &["TransformJobName"][..],
        "processing_job" => &["ProcessingJobName"][..],
        "pipeline" => &["PipelineName"][..],
        "domain" => &["DomainName", "DomainId"][..],
        "user_profile" => &["UserProfileName"][..],
        "feature_group" => &["FeatureGroupName"][..],
        _ => &[][..],
    };

    first_str(properties, keys)
        .map(String::from)
        .unwrap_or_else(|| identifier.to_string())
}

fn resource_arn(
    resource_kind: &str,
    identifier: &str,
    properties: &Value,
    aws_account_dto: &AwsAccountDto,
) -> String {
    let keys = match resource_kind {
        "notebook_instance" => &["NotebookInstanceArn"][..],
        "endpoint" => &["EndpointArn"][..],
        "endpoint_config" => &["EndpointConfigArn"][..],
        "model" => &["ModelArn"][..],
        "training_job" => &["TrainingJobArn"][..],
        "transform_job" => &["TransformJobArn"][..],
        "processing_job" => &["ProcessingJobArn"][..],
        "pipeline" => &["PipelineArn"][..],
        "domain" => &["DomainArn"][..],
        "user_profile" => &["UserProfileArn"][..],
        "feature_group" => &["FeatureGroupArn"][..],
        _ => &[][..],
    };

    first_str(properties, keys)
        .map(String::from)
        .unwrap_or_else(|| {
            format!(
                "arn:aws:sagemaker:{}:{}:{}/{}",
                aws_account_dto.default_region,
                aws_account_dto.account_id,
                resource_kind,
                identifier
            )
        })
}

fn status(properties: &Value) -> Option<&str> {
    first_str(
        properties,
        &[
            "NotebookInstanceStatus",
            "EndpointStatus",
            "TrainingJobStatus",
            "TransformJobStatus",
            "ProcessingJobStatus",
            "PipelineStatus",
            "DomainStatus",
            "Status",
            "FeatureGroupStatus",
        ],
    )
}

fn insert_optional_str(
    resource_data: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        resource_data.insert(key.to_string(), json!(value));
    }
}

fn first_str<'a>(properties: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| properties.get(*key).and_then(|v| v.as_str()))
}

fn number_or_string(properties: &Value, key: &str) -> Option<Value> {
    let value = properties.get(key)?;
    if value.is_number() || value.is_string() {
        Some(value.clone())
    } else {
        None
    }
}

fn bool_value(properties: &Value, key: &str) -> Option<bool> {
    let value = properties.get(key)?;
    if let Some(value) = value.as_bool() {
        return Some(value);
    }
    value.as_str().and_then(|value| {
        if value.eq_ignore_ascii_case("true") {
            Some(true)
        } else if value.eq_ignore_ascii_case("false") {
            Some(false)
        } else {
            None
        }
    })
}

fn str_eq(properties: &Value, key: &str, expected: &str) -> bool {
    properties
        .get(key)
        .and_then(|v| v.as_str())
        .map(|value| value.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

fn vpc_configured(properties: &Value) -> bool {
    properties
        .get("VpcConfig")
        .is_some_and(|value| !value.is_null())
        || array_len(properties, "Subnets") > 0
        || array_len(properties, "SubnetIds") > 0
        || array_len(properties, "SecurityGroupIds") > 0
}

fn array_len(properties: &Value, key: &str) -> usize {
    properties
        .get(key)
        .and_then(|v| v.as_array())
        .map(|items| items.len())
        .unwrap_or(0)
}

fn count_array_object_with_key(properties: &Value, array_key: &str, object_key: &str) -> usize {
    properties
        .get(array_key)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get(object_key).is_some_and(|value| !value.is_null()))
                .count()
        })
        .unwrap_or(0)
}

fn sum_array_i64(properties: &Value, array_key: &str, item_key: &str) -> i64 {
    properties
        .get(array_key)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get(item_key)
                        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
                })
                .sum()
        })
        .unwrap_or(0)
}

fn data_capture_enabled(properties: &Value) -> bool {
    properties
        .get("DataCaptureConfig")
        .and_then(|config| config.get("EnableCapture"))
        .and_then(|enabled| {
            enabled.as_bool().or_else(|| {
                enabled.as_str().and_then(|value| {
                    if value.eq_ignore_ascii_case("true") {
                        Some(true)
                    } else if value.eq_ignore_ascii_case("false") {
                        Some(false)
                    } else {
                        None
                    }
                })
            })
        })
        .unwrap_or(false)
}

fn is_running_notebook_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "inservice" | "pending" | "updating"
    )
}

fn is_failed_status(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    status.contains("failed") || status.contains("error")
}

fn is_completed_job_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "completed" | "stopped" | "stopping"
    )
}

fn is_ready_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "inservice" | "ready" | "available" | "active"
    )
}

fn is_unhealthy_endpoint_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_lowercase().as_str(),
        "inservice" | "creating" | "updating" | "systemupdating" | "rollingback"
    )
}
