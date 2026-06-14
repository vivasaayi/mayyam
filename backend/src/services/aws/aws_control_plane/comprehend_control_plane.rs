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
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

const COMPREHEND_TYPES: &[ComprehendType] = &[
    ComprehendType {
        cloudcontrol_type: "AWS::Comprehend::DocumentClassifier",
        resource_kind: "document_classifier",
    },
    ComprehendType {
        cloudcontrol_type: "AWS::Comprehend::EntityRecognizer",
        resource_kind: "entity_recognizer",
    },
    ComprehendType {
        cloudcontrol_type: "AWS::Comprehend::Flywheel",
        resource_kind: "flywheel",
    },
];

pub struct ComprehendControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Copy, Clone, Debug)]
struct ComprehendType {
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
    document_classifier_count: usize,
    entity_recognizer_count: usize,
    flywheel_count: usize,
    custom_model_resource_count: usize,
    failed_resource_count: usize,
    incomplete_resource_count: usize,
    non_active_resource_count: usize,
    data_access_role_missing_count: usize,
    model_without_kms_count: usize,
    volume_without_kms_count: usize,
    output_without_kms_count: usize,
    data_lake_without_kms_count: usize,
    data_lake_missing_count: usize,
    vpc_config_missing_count: usize,
}

impl ComprehendControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Comprehend inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_cloudcontrol_client(aws_account_dto)
            .await?;
        let mut resources = Vec::new();
        let mut summary = CollectionSummary::default();

        for comprehend_type in COMPREHEND_TYPES {
            match list_cloudcontrol_resources(&client, comprehend_type.cloudcontrol_type).await {
                Ok(descriptions) => {
                    for description in descriptions {
                        resources.push(resource_from_description(
                            aws_account_dto,
                            sync_id,
                            comprehend_type,
                            &description,
                            &mut summary,
                        ));
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to list Comprehend resources for {} through Cloud Control: {}",
                        comprehend_type.cloudcontrol_type, e
                    );
                    summary.collection_error_count += 1;
                    summary.collection_errors.push(json!({
                        "cloudcontrol_type": comprehend_type.cloudcontrol_type,
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
    comprehend_type: &ComprehendType,
    description: &ResourceDescription,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let identifier = description.identifier().unwrap_or("unknown").to_string();
    let properties = parse_properties(description.properties());
    let tags = tags_from_properties(&properties);
    let resource_name = resource_name(comprehend_type.resource_kind, &identifier, &properties);

    summary.resource_count += 1;
    *summary
        .resources_by_kind
        .entry(comprehend_type.resource_kind.to_string())
        .or_insert(0) += 1;
    if tags.as_object().map(|tags| tags.is_empty()).unwrap_or(true) {
        summary.untagged_resource_count += 1;
    }
    record_comprehend_evidence_counts(comprehend_type.resource_kind, &properties, summary);

    let mut resource_data = Map::new();
    resource_data.insert(
        "resource_kind".to_string(),
        json!(comprehend_type.resource_kind),
    );
    resource_data.insert(
        "cloudcontrol_type".to_string(),
        json!(comprehend_type.cloudcontrol_type),
    );
    resource_data.insert("cloudcontrol_identifier".to_string(), json!(identifier));
    resource_data.insert("properties".to_string(), properties.clone());
    insert_optional_str(&mut resource_data, "status", status(&properties));
    insert_optional_str(
        &mut resource_data,
        "status_message",
        first_str(&properties, &["StatusMessage", "Message", "FailureReason"]),
    );
    insert_optional_str(
        &mut resource_data,
        "version_name",
        first_str(&properties, &["VersionName"]),
    );
    insert_optional_str(
        &mut resource_data,
        "language_code",
        first_str(&properties, &["LanguageCode"]),
    );
    insert_optional_str(
        &mut resource_data,
        "mode",
        first_str(&properties, &["Mode"]),
    );
    insert_optional_str(
        &mut resource_data,
        "model_type",
        first_str(&properties, &["ModelType"]),
    );
    insert_optional_str(
        &mut resource_data,
        "data_access_role_arn",
        data_access_role_arn(&properties),
    );
    insert_optional_str(
        &mut resource_data,
        "model_kms_key_id",
        model_kms_key_id(&properties),
    );
    insert_optional_str(
        &mut resource_data,
        "volume_kms_key_id",
        volume_kms_key_id(&properties),
    );
    insert_optional_str(
        &mut resource_data,
        "output_s3_uri",
        nested_str(&properties, "OutputDataConfig", &["S3Uri", "S3URI"]),
    );
    resource_data.insert(
        "output_configured".to_string(),
        json!(output_configured(&properties)),
    );
    resource_data.insert(
        "output_kms_configured".to_string(),
        json!(output_kms_configured(&properties)),
    );
    insert_optional_str(
        &mut resource_data,
        "data_lake_s3_uri",
        first_str(&properties, &["DataLakeS3Uri", "DataLakeS3URI"]),
    );
    resource_data.insert(
        "data_lake_kms_configured".to_string(),
        json!(data_lake_kms_configured(&properties)),
    );
    resource_data.insert(
        "input_configured".to_string(),
        json!(object_configured(&properties, "InputDataConfig")),
    );
    resource_data.insert(
        "input_s3_uri_count".to_string(),
        json!(input_s3_uri_count(&properties)),
    );
    resource_data.insert(
        "vpc_configured".to_string(),
        json!(vpc_configured(&properties)),
    );
    resource_data.insert(
        "subnet_count".to_string(),
        json!(nested_array_len(&properties, "VpcConfig", "Subnets")),
    );
    resource_data.insert(
        "security_group_count".to_string(),
        json!(nested_array_len(
            &properties,
            "VpcConfig",
            "SecurityGroupIds"
        )),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::ComprehendResource.to_string(),
        resource_id: format!("{}/{}", comprehend_type.resource_kind, resource_name),
        arn: resource_arn(
            comprehend_type.resource_kind,
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
    let mut resource_data = Map::new();
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
        "document_classifier_count".to_string(),
        json!(summary.document_classifier_count),
    );
    resource_data.insert(
        "entity_recognizer_count".to_string(),
        json!(summary.entity_recognizer_count),
    );
    resource_data.insert("flywheel_count".to_string(), json!(summary.flywheel_count));
    resource_data.insert(
        "custom_model_resource_count".to_string(),
        json!(summary.custom_model_resource_count),
    );
    resource_data.insert(
        "failed_resource_count".to_string(),
        json!(summary.failed_resource_count),
    );
    resource_data.insert(
        "incomplete_resource_count".to_string(),
        json!(summary.incomplete_resource_count),
    );
    resource_data.insert(
        "non_active_resource_count".to_string(),
        json!(summary.non_active_resource_count),
    );
    resource_data.insert(
        "data_access_role_missing_count".to_string(),
        json!(summary.data_access_role_missing_count),
    );
    resource_data.insert(
        "model_without_kms_count".to_string(),
        json!(summary.model_without_kms_count),
    );
    resource_data.insert(
        "volume_without_kms_count".to_string(),
        json!(summary.volume_without_kms_count),
    );
    resource_data.insert(
        "output_without_kms_count".to_string(),
        json!(summary.output_without_kms_count),
    );
    resource_data.insert(
        "data_lake_without_kms_count".to_string(),
        json!(summary.data_lake_without_kms_count),
    );
    resource_data.insert(
        "data_lake_missing_count".to_string(),
        json!(summary.data_lake_missing_count),
    );
    resource_data.insert(
        "vpc_config_missing_count".to_string(),
        json!(summary.vpc_config_missing_count),
    );
    resource_data.insert(
        "cloudcontrol_type_count".to_string(),
        json!(COMPREHEND_TYPES.len()),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::ComprehendResource.to_string(),
        resource_id: format!("comprehend:{}", aws_account_dto.account_id),
        arn: format!(
            "arn:aws:comprehend:{}:{}:account/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
        ),
        name: Some("Comprehend".to_string()),
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

    let mut tag_map = Map::new();
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

fn record_comprehend_evidence_counts(
    resource_kind: &str,
    properties: &Value,
    summary: &mut CollectionSummary,
) {
    match resource_kind {
        "document_classifier" => summary.document_classifier_count += 1,
        "entity_recognizer" => summary.entity_recognizer_count += 1,
        "flywheel" => summary.flywheel_count += 1,
        _ => {}
    }

    if is_custom_model_resource(resource_kind) {
        summary.custom_model_resource_count += 1;
    }

    if let Some(status) = status(properties) {
        if is_failed_status(status) {
            summary.failed_resource_count += 1;
        }
        if !is_completed_status(status) {
            summary.non_active_resource_count += 1;
            if !is_failed_status(status) {
                summary.incomplete_resource_count += 1;
            }
        }
    }

    if data_access_role_arn(properties).is_none() {
        summary.data_access_role_missing_count += 1;
    }
    if model_kms_key_id(properties).is_none() {
        summary.model_without_kms_count += 1;
    }
    if resource_kind != "flywheel" && volume_kms_key_id(properties).is_none() {
        summary.volume_without_kms_count += 1;
    }
    if output_configured(properties) && !output_kms_configured(properties) {
        summary.output_without_kms_count += 1;
    }
    if resource_kind == "flywheel" {
        if first_str(properties, &["DataLakeS3Uri", "DataLakeS3URI"]).is_none() {
            summary.data_lake_missing_count += 1;
        } else if !data_lake_kms_configured(properties) {
            summary.data_lake_without_kms_count += 1;
        }
    }
    if resource_kind != "flywheel" && !vpc_configured(properties) {
        summary.vpc_config_missing_count += 1;
    }
}

fn resource_name(resource_kind: &str, identifier: &str, properties: &Value) -> String {
    let keys = match resource_kind {
        "document_classifier" => &["DocumentClassifierName", "Name"][..],
        "entity_recognizer" => &["EntityRecognizerName", "Name"][..],
        "flywheel" => &["FlywheelName", "Name"][..],
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
        "document_classifier" => &["DocumentClassifierArn", "Arn"][..],
        "entity_recognizer" => &["EntityRecognizerArn", "Arn"][..],
        "flywheel" => &["FlywheelArn", "Arn"][..],
        _ => &[][..],
    };

    first_str(properties, keys)
        .map(String::from)
        .unwrap_or_else(|| {
            format!(
                "arn:aws:comprehend:{}:{}:{}/{}",
                aws_account_dto.default_region,
                aws_account_dto.account_id,
                arn_resource_path(resource_kind),
                identifier
            )
        })
}

fn arn_resource_path(resource_kind: &str) -> &str {
    match resource_kind {
        "document_classifier" => "document-classifier",
        "entity_recognizer" => "entity-recognizer",
        "flywheel" => "flywheel",
        _ => resource_kind,
    }
}

fn status(properties: &Value) -> Option<&str> {
    first_str(
        properties,
        &[
            "Status",
            "DocumentClassifierStatus",
            "EntityRecognizerStatus",
            "FlywheelStatus",
            "ModelStatus",
        ],
    )
}

fn data_access_role_arn(properties: &Value) -> Option<&str> {
    first_str(properties, &["DataAccessRoleArn", "RoleArn"])
}

fn model_kms_key_id(properties: &Value) -> Option<&str> {
    first_str(properties, &["ModelKmsKeyId", "ModelKMSKeyId", "KmsKeyId"]).or_else(|| {
        nested_str(
            properties,
            "DataSecurityConfig",
            &["ModelKmsKeyId", "ModelKMSKeyId", "KmsKeyId"],
        )
    })
}

fn volume_kms_key_id(properties: &Value) -> Option<&str> {
    first_str(properties, &["VolumeKmsKeyId", "VolumeKMSKeyId"]).or_else(|| {
        nested_str(
            properties,
            "DataSecurityConfig",
            &["VolumeKmsKeyId", "VolumeKMSKeyId"],
        )
    })
}

fn output_configured(properties: &Value) -> bool {
    object_configured(properties, "OutputDataConfig")
}

fn output_kms_configured(properties: &Value) -> bool {
    nested_str(
        properties,
        "OutputDataConfig",
        &["KmsKeyId", "KMSKeyId", "KmsKeyID"],
    )
    .is_some()
}

fn data_lake_kms_configured(properties: &Value) -> bool {
    nested_str(
        properties,
        "DataSecurityConfig",
        &["DataLakeKmsKeyId", "DataLakeKMSKeyId", "KmsKeyId"],
    )
    .is_some()
}

fn vpc_configured(properties: &Value) -> bool {
    object_configured(properties, "VpcConfig")
}

fn input_s3_uri_count(properties: &Value) -> usize {
    let Some(input) = properties.get("InputDataConfig") else {
        return 0;
    };

    let mut count = 0usize;
    if first_str(input, &["S3Uri", "S3URI"]).is_some() {
        count += 1;
    }
    for key in ["Documents", "Annotations", "EntityList"] {
        if nested_str(input, key, &["S3Uri", "S3URI"]).is_some() {
            count += 1;
        }
    }
    count + array_len(input, "AugmentedManifests")
}

fn insert_optional_str(resource_data: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        resource_data.insert(key.to_string(), json!(value));
    }
}

fn first_str<'a>(properties: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| properties.get(*key).and_then(|v| v.as_str()))
}

fn nested_str<'a>(properties: &'a Value, parent: &str, keys: &[&str]) -> Option<&'a str> {
    let parent = properties.get(parent)?;
    keys.iter()
        .find_map(|key| parent.get(*key).and_then(|v| v.as_str()))
}

fn object_configured(properties: &Value, key: &str) -> bool {
    properties
        .get(key)
        .and_then(|value| value.as_object())
        .map(|object| !object.is_empty())
        .unwrap_or(false)
}

fn array_len(properties: &Value, key: &str) -> usize {
    properties
        .get(key)
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0)
}

fn nested_array_len(properties: &Value, parent: &str, key: &str) -> usize {
    properties
        .get(parent)
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0)
}

fn is_custom_model_resource(resource_kind: &str) -> bool {
    matches!(
        resource_kind,
        "document_classifier" | "entity_recognizer" | "flywheel"
    )
}

fn is_failed_status(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    status.contains("failed") || status.contains("error")
}

fn is_completed_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "active"
            | "available"
            | "ready"
            | "trained"
            | "trained_with_warning"
            | "succeeded"
            | "completed"
    )
}
