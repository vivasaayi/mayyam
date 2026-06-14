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

const TEXTRACT_TYPES: &[TextractType] = &[
    TextractType {
        cloudcontrol_type: "AWS::Textract::Adapter",
        resource_kind: "adapter",
    },
    TextractType {
        cloudcontrol_type: "AWS::Textract::AdapterVersion",
        resource_kind: "adapter_version",
    },
];

pub struct TextractControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Copy, Clone, Debug)]
struct TextractType {
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
    adapter_count: usize,
    adapter_version_count: usize,
    inactive_adapter_count: usize,
    adapter_without_versions_count: usize,
    failed_adapter_version_count: usize,
    at_risk_adapter_version_count: usize,
    non_active_adapter_version_count: usize,
    incomplete_adapter_version_count: usize,
    adapter_version_without_kms_count: usize,
    output_without_kms_count: usize,
    output_config_missing_count: usize,
    auto_update_disabled_count: usize,
}

impl TextractControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Textract inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_cloudcontrol_client(aws_account_dto)
            .await?;
        let mut descriptions_by_type: Vec<(TextractType, ResourceDescription)> = Vec::new();
        let mut resources = Vec::new();
        let mut summary = CollectionSummary::default();

        for textract_type in TEXTRACT_TYPES {
            match list_cloudcontrol_resources(&client, textract_type.cloudcontrol_type).await {
                Ok(descriptions) => {
                    for description in descriptions {
                        descriptions_by_type.push((*textract_type, description));
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to list Textract resources for {} through Cloud Control: {}",
                        textract_type.cloudcontrol_type, e
                    );
                    summary.collection_error_count += 1;
                    summary.collection_errors.push(json!({
                        "cloudcontrol_type": textract_type.cloudcontrol_type,
                        "error": e,
                    }));
                }
            }
        }

        let adapter_version_counts = adapter_version_counts_by_adapter(&descriptions_by_type);
        for (textract_type, description) in descriptions_by_type {
            resources.push(resource_from_description(
                aws_account_dto,
                sync_id,
                &textract_type,
                &description,
                &adapter_version_counts,
                &mut summary,
            ));
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
    textract_type: &TextractType,
    description: &ResourceDescription,
    adapter_version_counts: &BTreeMap<String, usize>,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let identifier = description.identifier().unwrap_or("unknown").to_string();
    let properties = parse_properties(description.properties());
    let tags = tags_from_properties(&properties);
    let resource_name = resource_name(textract_type.resource_kind, &identifier, &properties);
    let adapter_id = adapter_id(&properties, &identifier);

    summary.resource_count += 1;
    *summary
        .resources_by_kind
        .entry(textract_type.resource_kind.to_string())
        .or_insert(0) += 1;
    if tags.as_object().map(|tags| tags.is_empty()).unwrap_or(true) {
        summary.untagged_resource_count += 1;
    }
    record_textract_evidence_counts(textract_type.resource_kind, &properties, summary);

    let mut resource_data = Map::new();
    resource_data.insert(
        "resource_kind".to_string(),
        json!(textract_type.resource_kind),
    );
    resource_data.insert(
        "cloudcontrol_type".to_string(),
        json!(textract_type.cloudcontrol_type),
    );
    resource_data.insert("cloudcontrol_identifier".to_string(), json!(identifier));
    resource_data.insert("properties".to_string(), properties.clone());
    insert_optional_str(&mut resource_data, "status", status(&properties));
    insert_optional_str(&mut resource_data, "adapter_id", adapter_id.as_deref());
    insert_optional_str(
        &mut resource_data,
        "adapter_name",
        first_str(&properties, &["AdapterName", "Name"]),
    );
    insert_optional_str(
        &mut resource_data,
        "adapter_version",
        first_str(&properties, &["AdapterVersion", "Version"]),
    );
    insert_optional_str(
        &mut resource_data,
        "status_message",
        first_str(
            &properties,
            &["StatusMessage", "AdapterVersionStatusMessage", "Message"],
        ),
    );
    insert_optional_str(
        &mut resource_data,
        "data_access_role_arn",
        first_str(&properties, &["DataAccessRoleArn", "RoleArn"]),
    );
    insert_optional_str(&mut resource_data, "kms_key_id", kms_key_id(&properties));

    if let Some(auto_update) = properties.get("AutoUpdate") {
        resource_data.insert("auto_update".to_string(), auto_update.clone());
    }
    resource_data.insert(
        "auto_update_disabled".to_string(),
        json!(auto_update_disabled(&properties)),
    );
    resource_data.insert(
        "feature_type_count".to_string(),
        json!(array_len(&properties, "FeatureTypes")),
    );
    if let Some(feature_types) = properties.get("FeatureTypes") {
        resource_data.insert("feature_types".to_string(), feature_types.clone());
    }
    resource_data.insert(
        "output_configured".to_string(),
        json!(output_configured(&properties)),
    );
    insert_optional_str(
        &mut resource_data,
        "output_s3_bucket",
        nested_str(&properties, "OutputConfig", &["S3Bucket", "S3BucketName"]),
    );
    insert_optional_str(
        &mut resource_data,
        "output_s3_prefix",
        nested_str(&properties, "OutputConfig", &["S3Prefix", "S3KeyPrefix"]),
    );
    resource_data.insert(
        "output_kms_configured".to_string(),
        json!(output_kms_configured(&properties)),
    );
    if textract_type.resource_kind == "adapter" {
        let adapter_version_count = adapter_id
            .as_ref()
            .and_then(|id| adapter_version_counts.get(id))
            .copied()
            .unwrap_or(0);
        resource_data.insert(
            "adapter_version_count".to_string(),
            json!(adapter_version_count),
        );
        if adapter_version_count == 0 {
            summary.adapter_without_versions_count += 1;
        }
    }

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::TextractResource.to_string(),
        resource_id: format!("{}/{}", textract_type.resource_kind, resource_name),
        arn: resource_arn(
            textract_type.resource_kind,
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
    resource_data.insert("adapter_count".to_string(), json!(summary.adapter_count));
    resource_data.insert(
        "adapter_version_count".to_string(),
        json!(summary.adapter_version_count),
    );
    resource_data.insert(
        "inactive_adapter_count".to_string(),
        json!(summary.inactive_adapter_count),
    );
    resource_data.insert(
        "adapter_without_versions_count".to_string(),
        json!(summary.adapter_without_versions_count),
    );
    resource_data.insert(
        "failed_adapter_version_count".to_string(),
        json!(summary.failed_adapter_version_count),
    );
    resource_data.insert(
        "at_risk_adapter_version_count".to_string(),
        json!(summary.at_risk_adapter_version_count),
    );
    resource_data.insert(
        "non_active_adapter_version_count".to_string(),
        json!(summary.non_active_adapter_version_count),
    );
    resource_data.insert(
        "incomplete_adapter_version_count".to_string(),
        json!(summary.incomplete_adapter_version_count),
    );
    resource_data.insert(
        "adapter_version_without_kms_count".to_string(),
        json!(summary.adapter_version_without_kms_count),
    );
    resource_data.insert(
        "output_without_kms_count".to_string(),
        json!(summary.output_without_kms_count),
    );
    resource_data.insert(
        "output_config_missing_count".to_string(),
        json!(summary.output_config_missing_count),
    );
    resource_data.insert(
        "auto_update_disabled_count".to_string(),
        json!(summary.auto_update_disabled_count),
    );
    resource_data.insert(
        "cloudcontrol_type_count".to_string(),
        json!(TEXTRACT_TYPES.len()),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::TextractResource.to_string(),
        resource_id: format!("textract:{}", aws_account_dto.account_id),
        arn: format!(
            "arn:aws:textract:{}:{}:account/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
        ),
        name: Some("Textract".to_string()),
        tags: json!({}),
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn adapter_version_counts_by_adapter(
    descriptions_by_type: &[(TextractType, ResourceDescription)],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for (textract_type, description) in descriptions_by_type {
        if textract_type.resource_kind != "adapter_version" {
            continue;
        }
        let identifier = description.identifier().unwrap_or("unknown");
        let properties = parse_properties(description.properties());
        if let Some(adapter_id) = adapter_id(&properties, identifier) {
            *counts.entry(adapter_id).or_insert(0) += 1;
        }
    }
    counts
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

fn record_textract_evidence_counts(
    resource_kind: &str,
    properties: &Value,
    summary: &mut CollectionSummary,
) {
    match resource_kind {
        "adapter" => {
            summary.adapter_count += 1;
            if status(properties)
                .map(|status| !is_active_status(status))
                .unwrap_or(false)
            {
                summary.inactive_adapter_count += 1;
            }
            if auto_update_disabled(properties) {
                summary.auto_update_disabled_count += 1;
            }
        }
        "adapter_version" => {
            summary.adapter_version_count += 1;
            if let Some(status) = status(properties) {
                if is_failed_status(status) {
                    summary.failed_adapter_version_count += 1;
                }
                if is_at_risk_status(status) {
                    summary.at_risk_adapter_version_count += 1;
                }
                if !is_completed_version_status(status) {
                    summary.non_active_adapter_version_count += 1;
                    if !is_failed_status(status) {
                        summary.incomplete_adapter_version_count += 1;
                    }
                }
            }
            if kms_key_id(properties).is_none() {
                summary.adapter_version_without_kms_count += 1;
            }
            if output_configured(properties) {
                if !output_kms_configured(properties) {
                    summary.output_without_kms_count += 1;
                }
            } else {
                summary.output_config_missing_count += 1;
            }
        }
        _ => {}
    }
}

fn resource_name(resource_kind: &str, identifier: &str, properties: &Value) -> String {
    let keys = match resource_kind {
        "adapter" => &["AdapterName", "Name", "AdapterId"][..],
        "adapter_version" => &["AdapterVersion", "Version", "AdapterId"][..],
        _ => &[][..],
    };

    first_str(properties, keys)
        .map(String::from)
        .unwrap_or_else(|| identifier.to_string())
}

fn adapter_id(properties: &Value, identifier: &str) -> Option<String> {
    first_str(properties, &["AdapterId", "AdapterID"])
        .map(String::from)
        .or_else(|| adapter_id_from_identifier(identifier))
}

fn adapter_id_from_identifier(identifier: &str) -> Option<String> {
    let identifier = identifier.trim();
    if identifier.is_empty() || identifier == "unknown" {
        return None;
    }

    if let Some(after_adapter) = identifier.split("adapter/").nth(1) {
        let adapter_id = after_adapter
            .split(['/', '|'])
            .next()
            .unwrap_or(after_adapter)
            .trim();
        if !adapter_id.is_empty() {
            return Some(adapter_id.to_string());
        }
    }

    for separator in ['/', '|'] {
        if let Some((adapter_id, _)) = identifier.split_once(separator) {
            let adapter_id = adapter_id.trim();
            if !adapter_id.is_empty() {
                return Some(adapter_id.to_string());
            }
        }
    }

    None
}

fn resource_arn(
    resource_kind: &str,
    identifier: &str,
    properties: &Value,
    aws_account_dto: &AwsAccountDto,
) -> String {
    let keys = match resource_kind {
        "adapter" => &["AdapterArn", "Arn"][..],
        "adapter_version" => &["AdapterVersionArn", "Arn"][..],
        _ => &[][..],
    };

    first_str(properties, keys)
        .map(String::from)
        .unwrap_or_else(|| {
            format!(
                "arn:aws:textract:{}:{}:{}/{}",
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
            "AdapterStatus",
            "AdapterVersionStatus",
            "Status",
            "CreationStatus",
        ],
    )
}

fn kms_key_id(properties: &Value) -> Option<&str> {
    first_str(properties, &["KmsKeyId", "KMSKeyId", "KmsKeyID"]).or_else(|| {
        nested_str(
            properties,
            "OutputConfig",
            &["KmsKeyId", "KMSKeyId", "KmsKeyID"],
        )
    })
}

fn output_configured(properties: &Value) -> bool {
    properties
        .get("OutputConfig")
        .and_then(|value| value.as_object())
        .map(|object| !object.is_empty())
        .unwrap_or(false)
}

fn output_kms_configured(properties: &Value) -> bool {
    nested_str(
        properties,
        "OutputConfig",
        &["KmsKeyId", "KMSKeyId", "KmsKeyID"],
    )
    .is_some()
        || first_str(properties, &["KmsKeyId", "KMSKeyId", "KmsKeyID"]).is_some()
}

fn auto_update_disabled(properties: &Value) -> bool {
    match properties.get("AutoUpdate") {
        Some(Value::Bool(enabled)) => !enabled,
        Some(Value::String(value)) => matches!(
            value.to_ascii_lowercase().as_str(),
            "disabled" | "disable" | "false" | "off"
        ),
        _ => false,
    }
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

fn array_len(properties: &Value, key: &str) -> usize {
    properties
        .get(key)
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0)
}

fn is_failed_status(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    status.contains("failed") || status.contains("error")
}

fn is_at_risk_status(status: &str) -> bool {
    status.eq_ignore_ascii_case("at_risk") || status.eq_ignore_ascii_case("at risk")
}

fn is_active_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "active" | "available" | "ready" | "inservice" | "succeeded" | "completed"
    )
}

fn is_completed_version_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "active" | "available" | "ready" | "succeeded" | "completed"
    )
}
