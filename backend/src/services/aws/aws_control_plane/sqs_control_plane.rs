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
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use aws_sdk_sqs::Client as SqsClient;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct SqsControlPlane {
    aws_service: Arc<AwsService>,
}

impl SqsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_queues(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing SQS queues for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let client = self.aws_service.create_sqs_client(aws_account_dto).await?;

        // List all queues from AWS
        let response = client.list_queues().send().await.map_err(|e| {
            error!("Failed to list SQS queues: {}", e);
            let inner_aws_error = e.into_service_error();
            error!("Error raw response: {:?}", inner_aws_error);
            AppError::ExternalService(format!("Failed to list SQS queues: {}", inner_aws_error))
        })?;

        let mut queues = Vec::new();

        debug!("Fetched {} SQS queues", response.queue_urls().len());

        for queue_url in response.queue_urls() {
            // Extract queue name from URL
            let queue_name = queue_url.split('/').last().unwrap_or_default();

            debug!("Found SQS queue: {}", &queue_name);

            // Get queue attributes
            let attributes_response = client
                .get_queue_attributes()
                .queue_url(queue_url)
                .attribute_names(aws_sdk_sqs::types::QueueAttributeName::All)
                .send()
                .await
                .map_err(|e| {
                    AppError::ExternalService(format!(
                        "Failed to get attributes for queue {}: {}",
                        queue_url, e
                    ))
                })?;

            // Copy attributes to a local HashMap for easier use
            let mut attributes = std::collections::HashMap::new();
            if let Some(attrs) = attributes_response.attributes() {
                for (key, value) in attrs {
                    attributes.insert(key.as_str().to_string(), value.clone());
                }
            }

            // Get queue tags
            let tags_response = client
                .list_queue_tags()
                .queue_url(queue_url)
                .send()
                .await
                .map_err(|e| {
                    AppError::ExternalService(format!(
                        "Failed to get tags for queue {}: {}",
                        queue_url, e
                    ))
                })?;

            // Process tags
            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            if let Some(tags) = tags_response.tags() {
                for (key, value) in tags {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }

            // If no name tag was found, use the queue name
            if name.is_none() {
                name = Some(queue_name.to_string());
            }

            // Get queue ARN from our local HashMap
            let queue_arn = attributes
                .get("QueueArn")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "FIX_ME".to_string());

            // Build resource data
            let mut resource_data = serde_json::Map::new();

            resource_data.insert("queue_url".to_string(), json!(queue_url));

            // Parse numeric attributes
            if let Some(retention_period) = attributes.get("MessageRetentionPeriod") {
                if let Ok(value) = retention_period.parse::<i64>() {
                    resource_data.insert("message_retention_period".to_string(), json!(value));
                }
            }

            if let Some(visibility_timeout) = attributes.get("VisibilityTimeout") {
                if let Ok(value) = visibility_timeout.parse::<i64>() {
                    resource_data.insert("visibility_timeout".to_string(), json!(value));
                }
            }

            if let Some(delay_seconds) = attributes.get("DelaySeconds") {
                if let Ok(value) = delay_seconds.parse::<i64>() {
                    resource_data.insert("delay_seconds".to_string(), json!(value));
                }
            }

            if let Some(wait_time) = attributes.get("ReceiveMessageWaitTimeSeconds") {
                if let Ok(value) = wait_time.parse::<i64>() {
                    resource_data.insert(
                        "receive_message_wait_time_seconds".to_string(),
                        json!(value),
                    );
                }
            }

            if let Some(max_size) = attributes.get("MaximumMessageSize") {
                if let Ok(value) = max_size.parse::<i64>() {
                    resource_data.insert("max_message_size".to_string(), json!(value));
                }
            }

            // Check boolean attributes
            let is_fifo = queue_name.ends_with(".fifo")
                || attributes
                    .get("FifoQueue")
                    .map(|v| v == "true")
                    .unwrap_or(false);
            resource_data.insert("fifo_queue".to_string(), json!(is_fifo));

            if let Some(dedup) = attributes.get("ContentBasedDeduplication") {
                resource_data.insert(
                    "content_based_deduplication".to_string(),
                    json!(dedup == "true"),
                );
            }

            // Create resource DTO
            let queue = AwsResourceDto {
                id: None,
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: "SqsQueue".to_string(),
                resource_id: queue_name.to_string(),
                arn: queue_arn,
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
                sync_id: Some(sync_id),
            };

            queues.push(queue);
        }

        Ok(queues.into_iter().map(|q| q.into()).collect())
    }
}
