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
use aws_sdk_rds::Client as RdsClient;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info, trace};
use uuid::Uuid;

// Control plane implementation for RDS
pub struct RdsControlPlane {
    aws_service: Arc<AwsService>,
}

impl RdsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing RDS instances for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let client = self.aws_service.create_rds_client(aws_account_dto).await?;

        // Get DB instances from AWS
        let response = client.describe_db_instances().send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to describe RDS instances: {}", e))
        })?;

        let mut instances = Vec::new();

        debug!("Fetched {} RDS instances", response.db_instances().len());

        for db_instance in response.db_instances() {
            let db_identifier = db_instance.db_instance_identifier().unwrap_or_default();
            debug!("Found RDS instance: {}", &db_identifier);

            let arn = db_instance.db_instance_arn().unwrap_or_default();

            // Get tags for this instance
            let tags_response = client
                .list_tags_for_resource()
                .resource_name(arn)
                .send()
                .await
                .map_err(|e| {
                    error!("Failed to describe RDS instances: {}", &e);
                    let inner_aws_error = e.into_service_error();
                    error!("Error raw response: {:?}", &inner_aws_error);

                    AppError::ExternalService(format!(
                        "Failed to get tags for RDS instance {}: {}",
                        db_identifier, inner_aws_error
                    ))
                })?;

            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            for tag in tags_response.tag_list() {
                // FIX ME
                // if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                //     if key == "Name" {
                //         name = Some(value.to_string());
                //     }
                //     tags_map.insert(key.to_string(), json!(value));
                // }
            }

            // If no name tag was found, use the identifier as name
            if name.is_none() {
                name = Some(db_identifier.to_string());
            }

            // Build resource data
            let mut resource_data = serde_json::Map::new();

            // Add basic instance information
            resource_data.insert("db_instance_identifier".to_string(), json!(db_identifier));

            if let Some(engine) = db_instance.engine() {
                resource_data.insert("engine".to_string(), json!(engine));
            }

            if let Some(version) = db_instance.engine_version() {
                resource_data.insert("engine_version".to_string(), json!(version));
            }

            if let Some(class) = db_instance.db_instance_class() {
                resource_data.insert("instance_class".to_string(), json!(class));
            }

            let storage = db_instance.allocated_storage();
            resource_data.insert("allocated_storage".to_string(), json!(storage));

            // Add endpoint information
            if let Some(endpoint) = db_instance.endpoint() {
                let mut endpoint_data = serde_json::Map::new();

                if let Some(address) = endpoint.address() {
                    endpoint_data.insert("address".to_string(), json!(address));
                }

                endpoint_data.insert("port".to_string(), json!(endpoint.port()));

                if let Some(hosted_zone_id) = endpoint.hosted_zone_id() {
                    endpoint_data.insert("hosted_zone_id".to_string(), json!(hosted_zone_id));
                }

                resource_data.insert("endpoint".to_string(), json!(endpoint_data));
            }

            if let Some(status) = db_instance.db_instance_status() {
                resource_data.insert("status".to_string(), json!(status));
            }

            if let Some(az) = db_instance.availability_zone() {
                resource_data.insert("availability_zone".to_string(), json!(az));
            }

            resource_data.insert("multi_az".to_string(), json!(db_instance.multi_az()));

            if let Some(storage_type) = db_instance.storage_type() {
                resource_data.insert("storage_type".to_string(), json!(storage_type));
            }

            resource_data.insert(
                "backup_retention_period".to_string(),
                json!(db_instance.backup_retention_period()),
            );

            // Create resource DTO
            let instance = AwsResourceDto {
                id: None,
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: "RdsInstance".to_string(),
                resource_id: db_identifier.to_string(),
                arn: arn.to_string(),
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
                sync_id: Some(sync_id),
            };

            instances.push(instance);
        }

        Ok(instances.into_iter().map(|i| i.into()).collect())
    }
}
