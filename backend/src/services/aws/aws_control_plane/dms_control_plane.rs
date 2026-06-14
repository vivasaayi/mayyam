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

const DMS_TYPES: &[DmsType] = &[
    DmsType {
        cloudcontrol_type: "AWS::DMS::ReplicationInstance",
        resource_kind: "replication_instance",
    },
    DmsType {
        cloudcontrol_type: "AWS::DMS::Endpoint",
        resource_kind: "endpoint",
    },
    DmsType {
        cloudcontrol_type: "AWS::DMS::ReplicationTask",
        resource_kind: "replication_task",
    },
    DmsType {
        cloudcontrol_type: "AWS::DMS::EventSubscription",
        resource_kind: "event_subscription",
    },
    DmsType {
        cloudcontrol_type: "AWS::DMS::Certificate",
        resource_kind: "certificate",
    },
    DmsType {
        cloudcontrol_type: "AWS::DMS::ReplicationConfig",
        resource_kind: "replication_config",
    },
];

pub struct DmsControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Copy, Clone, Debug)]
struct DmsType {
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
    publicly_accessible_instance_count: usize,
    single_az_instance_count: usize,
    endpoint_without_ssl_count: usize,
    task_logging_disabled_count: usize,
}

impl DmsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS DMS inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_cloudcontrol_client(aws_account_dto)
            .await?;
        let mut resources = Vec::new();
        let mut summary = CollectionSummary::default();

        for dms_type in DMS_TYPES {
            match list_cloudcontrol_resources(&client, dms_type.cloudcontrol_type).await {
                Ok(descriptions) => {
                    for description in descriptions {
                        let resource = resource_from_description(
                            aws_account_dto,
                            sync_id,
                            dms_type,
                            &description,
                            &mut summary,
                        );
                        resources.push(resource);
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to list DMS resources for {} through Cloud Control: {}",
                        dms_type.cloudcontrol_type, e
                    );
                    summary.collection_error_count += 1;
                    summary.collection_errors.push(json!({
                        "cloudcontrol_type": dms_type.cloudcontrol_type,
                        "error": e,
                    }));
                }
            }
        }

        let summary_resource = account_summary_resource(aws_account_dto, sync_id, &summary);
        resources.insert(0, summary_resource);

        debug!(
            "Successfully synced {} AWS DMS inventory resources for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
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
    dms_type: &DmsType,
    description: &ResourceDescription,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let identifier = description.identifier().unwrap_or("unknown").to_string();
    let properties = parse_properties(description.properties());
    let tags = tags_from_properties(&properties);
    let resource_name = resource_name(dms_type.resource_kind, &identifier, &properties);
    let arn = resource_arn(
        dms_type.resource_kind,
        &identifier,
        &properties,
        aws_account_dto,
    );

    summary.resource_count += 1;
    *summary
        .resources_by_kind
        .entry(dms_type.resource_kind.to_string())
        .or_insert(0) += 1;
    if tags.as_object().map(|tags| tags.is_empty()).unwrap_or(true) {
        summary.untagged_resource_count += 1;
    }
    record_dms_evidence_counts(dms_type.resource_kind, &properties, summary);

    let mut resource_data = serde_json::Map::new();
    resource_data.insert("resource_kind".to_string(), json!(dms_type.resource_kind));
    resource_data.insert(
        "cloudcontrol_type".to_string(),
        json!(dms_type.cloudcontrol_type),
    );
    resource_data.insert("cloudcontrol_identifier".to_string(), json!(identifier));
    resource_data.insert("properties".to_string(), properties.clone());
    if let Some(status) = first_str(&properties, &["Status", "ReplicationInstanceStatus"]) {
        resource_data.insert("status".to_string(), json!(status));
    }
    if let Some(engine_name) = first_str(&properties, &["EngineName"]) {
        resource_data.insert("engine_name".to_string(), json!(engine_name));
    }
    if let Some(endpoint_type) = first_str(&properties, &["EndpointType"]) {
        resource_data.insert("endpoint_type".to_string(), json!(endpoint_type));
    }
    if let Some(migration_type) = first_str(&properties, &["MigrationType"]) {
        resource_data.insert("migration_type".to_string(), json!(migration_type));
    }
    if let Some(instance_class) = first_str(&properties, &["ReplicationInstanceClass"]) {
        resource_data.insert(
            "replication_instance_class".to_string(),
            json!(instance_class),
        );
    }
    if let Some(allocated_storage) = number_or_string(&properties, "AllocatedStorage") {
        resource_data.insert("allocated_storage_gb".to_string(), allocated_storage);
    }
    if let Some(multi_az) = bool_value(&properties, "MultiAZ") {
        resource_data.insert("multi_az".to_string(), json!(multi_az));
    }
    if let Some(publicly_accessible) = bool_value(&properties, "PubliclyAccessible") {
        resource_data.insert(
            "publicly_accessible".to_string(),
            json!(publicly_accessible),
        );
    }
    if let Some(deletion_protection) = bool_value(&properties, "DeletionProtection") {
        resource_data.insert(
            "deletion_protection".to_string(),
            json!(deletion_protection),
        );
    }
    if let Some(ssl_mode) = first_str(&properties, &["SslMode"]) {
        resource_data.insert("ssl_mode".to_string(), json!(ssl_mode));
    }
    if let Some(kms_key_id) = first_str(&properties, &["KmsKeyId"]) {
        resource_data.insert("kms_key_id".to_string(), json!(kms_key_id));
    }
    if let Some(logging_enabled) = task_logging_enabled(&properties) {
        resource_data.insert("task_logging_enabled".to_string(), json!(logging_enabled));
    }

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::DmsResource.to_string(),
        resource_id: format!("{}/{}", dms_type.resource_kind, resource_name),
        arn,
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
        "publicly_accessible_instance_count".to_string(),
        json!(summary.publicly_accessible_instance_count),
    );
    resource_data.insert(
        "single_az_instance_count".to_string(),
        json!(summary.single_az_instance_count),
    );
    resource_data.insert(
        "endpoint_without_ssl_count".to_string(),
        json!(summary.endpoint_without_ssl_count),
    );
    resource_data.insert(
        "task_logging_disabled_count".to_string(),
        json!(summary.task_logging_disabled_count),
    );
    resource_data.insert(
        "cloudcontrol_type_count".to_string(),
        json!(DMS_TYPES.len()),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::DmsResource.to_string(),
        resource_id: format!("dms:{}", aws_account_dto.account_id),
        arn: format!(
            "arn:aws:dms:{}:{}:account/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
        ),
        name: Some("AWS DMS".to_string()),
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

fn record_dms_evidence_counts(
    resource_kind: &str,
    properties: &Value,
    summary: &mut CollectionSummary,
) {
    match resource_kind {
        "replication_instance" => {
            if bool_value(properties, "PubliclyAccessible") == Some(true) {
                summary.publicly_accessible_instance_count += 1;
            }
            if bool_value(properties, "MultiAZ") == Some(false) {
                summary.single_az_instance_count += 1;
            }
        }
        "endpoint" => {
            if first_str(properties, &["SslMode"])
                .map(|ssl_mode| ssl_mode.eq_ignore_ascii_case("none"))
                .unwrap_or(false)
            {
                summary.endpoint_without_ssl_count += 1;
            }
        }
        "replication_task" => {
            if task_logging_enabled(properties) == Some(false) {
                summary.task_logging_disabled_count += 1;
            }
        }
        _ => {}
    }
}

fn resource_name(resource_kind: &str, identifier: &str, properties: &Value) -> String {
    let keys = match resource_kind {
        "replication_instance" => &["ReplicationInstanceIdentifier"][..],
        "endpoint" => &["EndpointIdentifier"][..],
        "replication_task" => &["ReplicationTaskIdentifier"][..],
        "event_subscription" => &["SubscriptionName"][..],
        "certificate" => &["CertificateIdentifier"][..],
        "replication_config" => &["ReplicationConfigIdentifier"][..],
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
        "replication_instance" => &["ReplicationInstanceArn"][..],
        "endpoint" => &["EndpointArn"][..],
        "replication_task" => &["ReplicationTaskArn"][..],
        "event_subscription" => &["EventSubscriptionArn"][..],
        "certificate" => &["CertificateArn"][..],
        "replication_config" => &["ReplicationConfigArn"][..],
        _ => &[][..],
    };

    first_str(properties, keys)
        .map(String::from)
        .unwrap_or_else(|| {
            format!(
                "arn:aws:dms:{}:{}:{}/{}",
                aws_account_dto.default_region,
                aws_account_dto.account_id,
                resource_kind,
                identifier
            )
        })
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

fn task_logging_enabled(properties: &Value) -> Option<bool> {
    let settings = properties.get("ReplicationTaskSettings")?;
    if let Some(settings) = settings.as_object() {
        return logging_value_from_task_settings(&Value::Object(settings.clone()));
    }
    let settings = settings.as_str()?;
    serde_json::from_str::<Value>(settings)
        .ok()
        .and_then(|value| logging_value_from_task_settings(&value))
}

fn logging_value_from_task_settings(settings: &Value) -> Option<bool> {
    settings
        .get("Logging")
        .and_then(|logging| logging.get("EnableLogging"))
        .and_then(|enabled| enabled.as_bool())
}
