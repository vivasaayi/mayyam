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
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct Route53ControlPlane {
    aws_service: Arc<AwsService>,
}

impl Route53ControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_hosted_zones(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Route 53 hosted zones for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_route53_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        // Zone IDs (without the "/hostedzone/" prefix) that have a query
        // logging configuration. Collected up front so each zone can be
        // annotated without a per-zone API call.
        let mut query_logging_zone_ids: HashSet<String> = HashSet::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_query_logging_configs();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Route 53 query logging configs: {}", e);
                AppError::ExternalService(format!(
                    "Failed to list Route 53 query logging configs: {}",
                    e
                ))
            })?;

            for config in response.query_logging_configs() {
                let zone_id = config.hosted_zone_id();
                let zone_id = zone_id
                    .strip_prefix("/hostedzone/")
                    .unwrap_or(zone_id)
                    .to_string();
                query_logging_zone_ids.insert(zone_id);
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        let mut marker: Option<String> = None;

        loop {
            let mut request = client.list_hosted_zones();
            if let Some(token) = marker {
                request = request.marker(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Route 53 hosted zones: {}", e);
                AppError::ExternalService(format!("Failed to list Route 53 hosted zones: {}", e))
            })?;

            for zone in response.hosted_zones() {
                let raw_id = zone.id();
                let zone_id = raw_id
                    .strip_prefix("/hostedzone/")
                    .unwrap_or(raw_id)
                    .to_string();
                let name = zone.name().to_string();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("name".to_string(), json!(name));

                if let Some(config) = zone.config() {
                    resource_data.insert("private_zone".to_string(), json!(config.private_zone()));
                    if let Some(comment) = config.comment() {
                        resource_data.insert("comment".to_string(), json!(comment));
                    }
                } else {
                    resource_data.insert("private_zone".to_string(), json!(false));
                }

                if let Some(count) = zone.resource_record_set_count() {
                    resource_data.insert("resource_record_set_count".to_string(), json!(count));
                }

                resource_data.insert(
                    "caller_reference".to_string(),
                    json!(zone.caller_reference()),
                );

                resource_data.insert(
                    "query_logging_enabled".to_string(),
                    json!(query_logging_zone_ids.contains(&zone_id)),
                );

                let mut tags_map = serde_json::Map::new();
                match client
                    .list_tags_for_resource()
                    .resource_type(aws_sdk_route53::types::TagResourceType::Hostedzone)
                    .resource_id(&zone_id)
                    .send()
                    .await
                {
                    Ok(tags_response) => {
                        if let Some(tag_set) = tags_response.resource_tag_set() {
                            for tag in tag_set.tags() {
                                if let Some(key) = tag.key() {
                                    tags_map
                                        .insert(key.to_string(), json!(tag.value().unwrap_or("")));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!(
                            "Failed to list tags for Route 53 hosted zone {}: {}",
                            zone_id, e
                        );
                    }
                }

                let arn = format!("arn:aws:route53:::hostedzone/{}", zone_id);

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    // Route 53 is a global service; the account default region
                    // is used so rows stay consistent with other collectors.
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::Route53HostedZone.to_string(),
                    resource_id: zone_id,
                    arn,
                    name: Some(name),
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };

                resources.push(dto.into());
            }

            marker = if response.is_truncated() {
                response.next_marker().map(String::from)
            } else {
                None
            };
            if marker.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} Route 53 hosted zones for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
