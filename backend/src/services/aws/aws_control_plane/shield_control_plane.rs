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
use aws_sdk_shield::types::Subscription;
use aws_smithy_types::date_time::Format;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct ShieldControlPlane {
    aws_service: Arc<AwsService>,
}

impl ShieldControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_protections(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Shield protections for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_shield_client(aws_account_dto)
            .await?;
        let subscription_data = match client.describe_subscription().send().await {
            Ok(response) => response.subscription().map(subscription_to_json),
            Err(e) => {
                debug!(
                    "Shield subscription details unavailable for account {}: {}",
                    &aws_account_dto.account_id, e
                );
                None
            }
        };

        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_protections().max_results(100);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Shield protections: {}", e);
                AppError::ExternalService(format!("Failed to list Shield protections: {}", e))
            })?;

            for protection in response.protections() {
                let protection_id = match protection.id() {
                    Some(id) => id.to_string(),
                    None => {
                        debug!("Skipping Shield protection entry without an ID");
                        continue;
                    }
                };

                let arn = protection
                    .protection_arn()
                    .or_else(|| protection.resource_arn())
                    .unwrap_or_default()
                    .to_string();
                if arn.is_empty() {
                    debug!(
                        "Skipping Shield protection {} without an ARN",
                        protection_id
                    );
                    continue;
                }

                let tags = if let Some(protection_arn) = protection.protection_arn() {
                    list_tags(&client, protection_arn).await
                } else {
                    serde_json::Value::Object(Default::default())
                };

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("protection_id".to_string(), json!(protection_id));
                resource_data.insert("arn".to_string(), json!(arn));
                if let Some(protection_arn) = protection.protection_arn() {
                    resource_data.insert("protection_arn".to_string(), json!(protection_arn));
                }
                if let Some(name) = protection.name() {
                    resource_data.insert("name".to_string(), json!(name));
                }
                if let Some(resource_arn) = protection.resource_arn() {
                    resource_data.insert("protected_resource_arn".to_string(), json!(resource_arn));
                }
                resource_data.insert(
                    "health_check_ids".to_string(),
                    json!(protection.health_check_ids()),
                );
                resource_data.insert(
                    "health_check_count".to_string(),
                    json!(protection.health_check_ids().len()),
                );

                if let Some(config) =
                    protection.application_layer_automatic_response_configuration()
                {
                    resource_data.insert(
                        "automatic_response_status".to_string(),
                        json!(config.status().as_str()),
                    );
                    resource_data.insert(
                        "automatic_response_action".to_string(),
                        json!(config.action().and_then(response_action_name)),
                    );
                }

                if let Some(subscription) = &subscription_data {
                    resource_data.insert("subscription".to_string(), subscription.clone());
                    if let Some(auto_renew) = subscription.get("auto_renew") {
                        resource_data
                            .insert("subscription_auto_renew".to_string(), auto_renew.clone());
                    }
                    if let Some(status) = subscription.get("proactive_engagement_status") {
                        resource_data
                            .insert("proactive_engagement_status".to_string(), status.clone());
                    }
                }

                let name = protection
                    .name()
                    .map(String::from)
                    .or_else(|| Some(protection_id.clone()));

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::ShieldProtection.to_string(),
                    resource_id: protection_id,
                    arn,
                    name,
                    tags,
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
            "Successfully synced {} Shield protections for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn list_tags(client: &aws_sdk_shield::Client, protection_arn: &str) -> serde_json::Value {
    let mut tags_map = serde_json::Map::new();
    match client
        .list_tags_for_resource()
        .resource_arn(protection_arn)
        .send()
        .await
    {
        Ok(response) => {
            for tag in response.tags() {
                if let Some(key) = tag.key() {
                    tags_map.insert(key.to_string(), json!(tag.value().unwrap_or("")));
                }
            }
        }
        Err(e) => {
            debug!("Failed to list Shield tags for {}: {}", protection_arn, e);
        }
    }
    serde_json::Value::Object(tags_map)
}

fn response_action_name(action: &aws_sdk_shield::types::ResponseAction) -> Option<&'static str> {
    if action.block().is_some() {
        Some("BLOCK")
    } else if action.count().is_some() {
        Some("COUNT")
    } else {
        None
    }
}

fn subscription_to_json(subscription: &Subscription) -> serde_json::Value {
    json!({
        "subscription_arn": subscription.subscription_arn(),
        "start_time": fmt_date(subscription.start_time()),
        "end_time": fmt_date(subscription.end_time()),
        "time_commitment_in_seconds": subscription.time_commitment_in_seconds(),
        "auto_renew": subscription.auto_renew().map(|value| value.as_str()),
        "proactive_engagement_status": subscription
            .proactive_engagement_status()
            .map(|value| value.as_str()),
        "limits_count": subscription.limits().len(),
    })
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.map(|d| {
        d.fmt(Format::DateTime)
            .unwrap_or_else(|_| format!("{:?}", d))
    })
}
