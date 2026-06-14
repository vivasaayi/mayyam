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
use aws_smithy_types::date_time::Format;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct AmazonMqControlPlane {
    aws_service: Arc<AwsService>,
}

impl AmazonMqControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_brokers(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Amazon MQ brokers for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_amazonmq_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_brokers();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Amazon MQ brokers: {}", e);
                AppError::ExternalService(format!("Failed to list Amazon MQ brokers: {}", e))
            })?;

            for broker in response.broker_summaries() {
                let Some(broker_id) = broker.broker_id() else {
                    continue;
                };

                let details = client
                    .describe_broker()
                    .broker_id(broker_id)
                    .send()
                    .await
                    .map_err(|e| {
                        error!("Failed to describe Amazon MQ broker {}: {}", broker_id, e);
                        AppError::ExternalService(format!(
                            "Failed to describe Amazon MQ broker {}: {}",
                            broker_id, e
                        ))
                    })?;

                let arn = details
                    .broker_arn()
                    .or_else(|| broker.broker_arn())
                    .unwrap_or_default()
                    .to_string();
                if arn.is_empty() {
                    continue;
                }

                let broker_name = details
                    .broker_name()
                    .or_else(|| broker.broker_name())
                    .map(String::from);

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("broker_id".to_string(), json!(broker_id));
                if let Some(name) = broker_name.as_deref() {
                    resource_data.insert("broker_name".to_string(), json!(name));
                }
                if let Some(state) = details.broker_state().or_else(|| broker.broker_state()) {
                    resource_data.insert("broker_state".to_string(), json!(state.as_str()));
                }
                if let Some(mode) = details.deployment_mode() {
                    resource_data.insert("deployment_mode".to_string(), json!(mode.as_str()));
                }
                if let Some(engine_type) = details.engine_type() {
                    resource_data.insert("engine_type".to_string(), json!(engine_type.as_str()));
                }
                if let Some(engine_version) = details.engine_version() {
                    resource_data.insert("engine_version".to_string(), json!(engine_version));
                }
                if let Some(instance_type) = details.host_instance_type() {
                    resource_data.insert("host_instance_type".to_string(), json!(instance_type));
                }
                if let Some(storage_type) = details.storage_type() {
                    resource_data.insert("storage_type".to_string(), json!(storage_type.as_str()));
                }
                if let Some(created) = fmt_date(details.created()) {
                    resource_data.insert("created".to_string(), json!(created));
                }

                resource_data.insert(
                    "auto_minor_version_upgrade".to_string(),
                    json!(details.auto_minor_version_upgrade()),
                );
                resource_data.insert(
                    "publicly_accessible".to_string(),
                    json!(details.publicly_accessible()),
                );
                resource_data.insert(
                    "subnet_count".to_string(),
                    json!(details.subnet_ids().len()),
                );
                resource_data.insert(
                    "security_group_count".to_string(),
                    json!(details.security_groups().len()),
                );
                resource_data.insert("user_count".to_string(), json!(details.users().len()));

                if let Some(logs) = details.logs() {
                    resource_data.insert("general_logs_enabled".to_string(), json!(logs.general()));
                    resource_data.insert("audit_logs_enabled".to_string(), json!(logs.audit()));
                }

                if let Some(encryption) = details.encryption_options() {
                    resource_data.insert(
                        "use_aws_owned_key".to_string(),
                        json!(encryption.use_aws_owned_key()),
                    );
                    if let Some(kms_key_id) = encryption.kms_key_id() {
                        resource_data.insert("kms_key_id".to_string(), json!(kms_key_id));
                    }
                }

                let mut tags_map = serde_json::Map::new();
                if let Some(tags) = details.tags() {
                    for (key, value) in tags {
                        tags_map.insert(key.clone(), json!(value));
                    }
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::AmazonMqBroker.to_string(),
                    resource_id: broker_id.to_string(),
                    arn,
                    name: broker_name,
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };
                resources.push(dto.into());
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} Amazon MQ brokers for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.map(|d| {
        d.fmt(Format::DateTime)
            .unwrap_or_else(|_| format!("{:?}", d))
    })
}
