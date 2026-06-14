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
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct FirehoseControlPlane {
    aws_service: Arc<AwsService>,
}

impl FirehoseControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_delivery_streams(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Kinesis Data Firehose delivery streams for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_firehose_client(aws_account_dto)
            .await?;
        let mut resources = Vec::new();
        let mut exclusive_start: Option<String> = None;

        loop {
            let mut request = client.list_delivery_streams().limit(100);
            if let Some(start) = exclusive_start {
                request = request.exclusive_start_delivery_stream_name(start);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Firehose delivery streams: {}", e);
                AppError::ExternalService(format!(
                    "Failed to list Firehose delivery streams: {}",
                    e
                ))
            })?;

            for stream_name in response.delivery_stream_names() {
                match client
                    .describe_delivery_stream()
                    .delivery_stream_name(stream_name)
                    .send()
                    .await
                {
                    Ok(details) => {
                        if let Some(description) = details.delivery_stream_description() {
                            let arn = description.delivery_stream_arn().to_string();
                            if arn.is_empty() {
                                continue;
                            }

                            let mut resource_data = serde_json::Map::new();
                            resource_data.insert(
                                "delivery_stream_name".to_string(),
                                json!(description.delivery_stream_name()),
                            );
                            resource_data.insert(
                                "delivery_stream_status".to_string(),
                                json!(description.delivery_stream_status().as_str()),
                            );
                            resource_data.insert(
                                "delivery_stream_type".to_string(),
                                json!(description.delivery_stream_type().as_str()),
                            );
                            resource_data
                                .insert("version_id".to_string(), json!(description.version_id()));
                            resource_data.insert(
                                "destinations_count".to_string(),
                                json!(description.destinations().len()),
                            );
                            if let Some(failure) = description.failure_description() {
                                resource_data.insert(
                                    "failure_description".to_string(),
                                    json!(failure.details()),
                                );
                            }
                            if let Some(encryption) =
                                description.delivery_stream_encryption_configuration()
                            {
                                if let Some(status) = encryption.status() {
                                    resource_data.insert(
                                        "server_side_encryption_status".to_string(),
                                        json!(status.as_str()),
                                    );
                                }
                                if let Some(key_arn) = encryption.key_arn() {
                                    resource_data.insert("kms_key_arn".to_string(), json!(key_arn));
                                }
                            }

                            let mut cloudwatch_logging_enabled = false;
                            let mut s3_backup_mode: Option<String> = None;
                            for destination in description.destinations() {
                                if let Some(s3) = destination.extended_s3_destination_description()
                                {
                                    if let Some(logging) = s3.cloud_watch_logging_options() {
                                        cloudwatch_logging_enabled |=
                                            logging.enabled().unwrap_or(false);
                                    }
                                    if let Some(mode) = s3.s3_backup_mode() {
                                        s3_backup_mode = Some(mode.as_str().to_string());
                                    }
                                }
                                if let Some(s3) = destination.s3_destination_description() {
                                    if let Some(logging) = s3.cloud_watch_logging_options() {
                                        cloudwatch_logging_enabled |=
                                            logging.enabled().unwrap_or(false);
                                    }
                                }
                            }
                            resource_data.insert(
                                "cloudwatch_logging_enabled".to_string(),
                                json!(cloudwatch_logging_enabled),
                            );
                            if let Some(mode) = s3_backup_mode {
                                resource_data.insert("s3_backup_mode".to_string(), json!(mode));
                            }

                            let tags = list_firehose_tags(&client, stream_name).await;
                            let dto = AwsResourceDto {
                                id: None,
                                sync_id: Some(sync_id),
                                account_id: aws_account_dto.account_id.clone(),
                                profile: aws_account_dto.profile.clone(),
                                region: aws_account_dto.default_region.clone(),
                                resource_type: AwsResourceType::FirehoseDeliveryStream.to_string(),
                                resource_id: stream_name.to_string(),
                                arn,
                                name: Some(stream_name.to_string()),
                                tags,
                                resource_data: serde_json::Value::Object(resource_data),
                            };
                            resources.push(dto.into());
                        }
                    }
                    Err(e) => {
                        debug!(
                            "Failed to describe Firehose delivery stream {}: {}",
                            stream_name, e
                        );
                    }
                }
            }

            if response.has_more_delivery_streams() {
                exclusive_start = response.delivery_stream_names().last().map(String::from);
                if exclusive_start.is_none() {
                    break;
                }
            } else {
                break;
            }
        }

        debug!(
            "Successfully synced {} Firehose delivery streams for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn list_firehose_tags(
    client: &aws_sdk_firehose::Client,
    stream_name: &str,
) -> serde_json::Value {
    let mut tags_map = serde_json::Map::new();
    let mut exclusive_start: Option<String> = None;

    loop {
        let mut request = client
            .list_tags_for_delivery_stream()
            .delivery_stream_name(stream_name)
            .limit(50);
        if let Some(start) = exclusive_start {
            request = request.exclusive_start_tag_key(start);
        }

        match request.send().await {
            Ok(response) => {
                for tag in response.tags() {
                    tags_map.insert(tag.key().to_string(), json!(tag.value()));
                }
                if response.has_more_tags() {
                    exclusive_start = response.tags().last().map(|tag| tag.key().to_string());
                    if exclusive_start.is_none() {
                        break;
                    }
                } else {
                    break;
                }
            }
            Err(e) => {
                debug!("Failed to list Firehose tags for {}: {}", stream_name, e);
                break;
            }
        }
    }

    serde_json::Value::Object(tags_map)
}
