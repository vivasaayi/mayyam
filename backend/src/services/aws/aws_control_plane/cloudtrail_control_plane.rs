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

pub struct CloudTrailControlPlane {
    aws_service: Arc<AwsService>,
}

impl CloudTrailControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_trails(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing CloudTrail trails for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_cloudtrail_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        // Exclude shadow trails so every returned trail is owned by this
        // region; describe_trails is not paginated.
        let response = client
            .describe_trails()
            .include_shadow_trails(false)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to list CloudTrail trails: {}", e);
                AppError::ExternalService(format!("Failed to list CloudTrail trails: {}", e))
            })?;

        debug!("Fetched {} CloudTrail trails", response.trail_list().len());

        for trail in response.trail_list() {
            let trail_name = trail.name().unwrap_or("");
            let trail_arn = trail.trail_arn().unwrap_or("");
            if trail_name.is_empty() && trail_arn.is_empty() {
                debug!("Skipping CloudTrail trail with no name or ARN");
                continue;
            }
            debug!("Found CloudTrail trail: {}", trail_name);

            let mut resource_data = serde_json::Map::new();
            resource_data.insert("trail_arn".to_string(), json!(trail_arn));

            if let Some(home_region) = trail.home_region() {
                resource_data.insert("home_region".to_string(), json!(home_region));
            }

            if let Some(multi_region) = trail.is_multi_region_trail() {
                resource_data.insert("is_multi_region_trail".to_string(), json!(multi_region));
            }

            if let Some(org_trail) = trail.is_organization_trail() {
                resource_data.insert("is_organization_trail".to_string(), json!(org_trail));
            }

            if let Some(validation) = trail.log_file_validation_enabled() {
                resource_data.insert("log_file_validation_enabled".to_string(), json!(validation));
            }

            if let Some(kms_key_id) = trail.kms_key_id() {
                resource_data.insert("kms_key_id".to_string(), json!(kms_key_id));
            }

            if let Some(bucket) = trail.s3_bucket_name() {
                resource_data.insert("s3_bucket_name".to_string(), json!(bucket));
            }

            if let Some(global_events) = trail.include_global_service_events() {
                resource_data.insert(
                    "include_global_service_events".to_string(),
                    json!(global_events),
                );
            }

            // get_trail_status accepts the trail ARN; prefer it over the bare
            // name. On failure, leave is_logging absent so evaluators can
            // report the collection gap, and keep syncing the rest.
            let status_name = if !trail_arn.is_empty() {
                trail_arn
            } else {
                trail_name
            };
            match client.get_trail_status().name(status_name).send().await {
                Ok(status) => {
                    if let Some(is_logging) = status.is_logging() {
                        resource_data.insert("is_logging".to_string(), json!(is_logging));
                    }
                    if let Some(delivery_error) = status.latest_delivery_error() {
                        if !delivery_error.is_empty() {
                            resource_data
                                .insert("latest_delivery_error".to_string(), json!(delivery_error));
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to get status for CloudTrail trail {}: {}; logging state not collected",
                        trail_name, e
                    );
                }
            }

            // list_tags is region-bound; shadow trails are excluded above so
            // every trail here is home to this region. One ARN per call keeps
            // the response single-page. On failure persist empty tags and
            // continue.
            let tags = match client.list_tags().resource_id_list(trail_arn).send().await {
                Ok(tags_response) => {
                    let mut tags_map = serde_json::Map::new();
                    for resource_tag in tags_response.resource_tag_list() {
                        for tag in resource_tag.tags_list() {
                            tags_map
                                .insert(tag.key().to_string(), json!(tag.value().unwrap_or("")));
                        }
                    }
                    serde_json::Value::Object(tags_map)
                }
                Err(e) => {
                    debug!(
                        "Failed to list tags for CloudTrail trail {}: {}; persisting empty tags",
                        trail_name, e
                    );
                    json!({})
                }
            };

            let dto = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: AwsResourceType::CloudTrailTrail.to_string(),
                resource_id: trail_name.to_string(),
                arn: trail_arn.to_string(),
                name: Some(trail_name.to_string()),
                tags,
                resource_data: serde_json::Value::Object(resource_data),
            };

            resources.push(dto.into());
        }

        debug!(
            "Successfully synced {} CloudTrail trails for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
