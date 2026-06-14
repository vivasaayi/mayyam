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

pub struct PrivateLinkControlPlane {
    aws_service: Arc<AwsService>,
}

impl PrivateLinkControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_vpc_endpoints(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing PrivateLink VPC endpoints for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_vpc_endpoints().max_results(1000);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe PrivateLink VPC endpoints: {}", e);
                AppError::ExternalService(format!(
                    "Failed to describe PrivateLink VPC endpoints: {}",
                    e
                ))
            })?;

            for endpoint in response.vpc_endpoints() {
                let endpoint_id = match endpoint.vpc_endpoint_id() {
                    Some(id) => id.to_string(),
                    None => {
                        debug!("Skipping PrivateLink endpoint entry without an ID");
                        continue;
                    }
                };

                let arn = format!(
                    "arn:aws:ec2:{}:{}:vpc-endpoint/{}",
                    aws_account_dto.default_region, aws_account_dto.account_id, endpoint_id
                );

                let group_ids: Vec<String> = endpoint
                    .groups()
                    .iter()
                    .filter_map(|group| group.group_id().map(String::from))
                    .collect();
                let group_names: Vec<String> = endpoint
                    .groups()
                    .iter()
                    .filter_map(|group| group.group_name().map(String::from))
                    .collect();
                let dns_entries: Vec<serde_json::Value> = endpoint
                    .dns_entries()
                    .iter()
                    .map(|entry| {
                        json!({
                            "dns_name": entry.dns_name(),
                            "hosted_zone_id": entry.hosted_zone_id(),
                        })
                    })
                    .collect();

                let association_count = endpoint.subnet_ids().len()
                    + endpoint.route_table_ids().len()
                    + endpoint.network_interface_ids().len();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("vpc_endpoint_id".to_string(), json!(endpoint_id));
                resource_data.insert("arn".to_string(), json!(arn));
                if let Some(endpoint_type) = endpoint.vpc_endpoint_type() {
                    resource_data
                        .insert("endpoint_type".to_string(), json!(endpoint_type.as_str()));
                }
                if let Some(vpc_id) = endpoint.vpc_id() {
                    resource_data.insert("vpc_id".to_string(), json!(vpc_id));
                }
                if let Some(service_name) = endpoint.service_name() {
                    resource_data.insert("service_name".to_string(), json!(service_name));
                }
                if let Some(state) = endpoint.state() {
                    resource_data.insert("state".to_string(), json!(state.as_str()));
                }
                if let Some(owner_id) = endpoint.owner_id() {
                    resource_data.insert("owner_id".to_string(), json!(owner_id));
                }
                if let Some(ip_address_type) = endpoint.ip_address_type() {
                    resource_data.insert(
                        "ip_address_type".to_string(),
                        json!(ip_address_type.as_str()),
                    );
                }
                if let Some(created) = endpoint.creation_timestamp() {
                    let formatted = created
                        .fmt(Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", created));
                    resource_data.insert("creation_timestamp".to_string(), json!(formatted));
                }
                if let Some(service_region) = endpoint.service_region() {
                    resource_data.insert("service_region".to_string(), json!(service_region));
                }
                if let Some(failure_reason) = endpoint.failure_reason() {
                    resource_data.insert("failure_reason".to_string(), json!(failure_reason));
                }
                if let Some(last_error) = endpoint.last_error() {
                    resource_data.insert(
                        "last_error".to_string(),
                        json!({
                            "code": last_error.code(),
                            "message": last_error.message(),
                        }),
                    );
                }

                resource_data.insert(
                    "route_table_ids".to_string(),
                    json!(endpoint.route_table_ids()),
                );
                resource_data.insert("subnet_ids".to_string(), json!(endpoint.subnet_ids()));
                resource_data.insert(
                    "network_interface_ids".to_string(),
                    json!(endpoint.network_interface_ids()),
                );
                resource_data.insert("security_group_ids".to_string(), json!(group_ids));
                resource_data.insert("security_group_names".to_string(), json!(group_names));
                resource_data.insert("dns_entries".to_string(), json!(dns_entries));
                resource_data.insert(
                    "private_dns_enabled".to_string(),
                    json!(endpoint.private_dns_enabled().unwrap_or(false)),
                );
                resource_data.insert(
                    "requester_managed".to_string(),
                    json!(endpoint.requester_managed().unwrap_or(false)),
                );
                resource_data.insert(
                    "policy_document_present".to_string(),
                    json!(endpoint
                        .policy_document()
                        .map(|policy| !policy.trim().is_empty())
                        .unwrap_or(false)),
                );
                resource_data.insert(
                    "policy_document_length".to_string(),
                    json!(endpoint
                        .policy_document()
                        .map(|policy| policy.len())
                        .unwrap_or(0)),
                );
                resource_data.insert(
                    "route_table_count".to_string(),
                    json!(endpoint.route_table_ids().len()),
                );
                resource_data.insert(
                    "subnet_count".to_string(),
                    json!(endpoint.subnet_ids().len()),
                );
                resource_data.insert(
                    "network_interface_count".to_string(),
                    json!(endpoint.network_interface_ids().len()),
                );
                resource_data.insert(
                    "security_group_count".to_string(),
                    json!(endpoint.groups().len()),
                );
                resource_data.insert(
                    "dns_entry_count".to_string(),
                    json!(endpoint.dns_entries().len()),
                );
                resource_data.insert("association_count".to_string(), json!(association_count));

                let mut tags_map = serde_json::Map::new();
                for tag in endpoint.tags() {
                    if let Some(key) = tag.key() {
                        tags_map.insert(key.to_string(), json!(tag.value().unwrap_or("")));
                    }
                }
                let name = tags_map
                    .get("Name")
                    .and_then(|value| value.as_str())
                    .map(String::from)
                    .or_else(|| Some(endpoint_id.clone()));

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::VpcEndpoint.to_string(),
                    resource_id: endpoint_id,
                    arn,
                    name,
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
            "Successfully synced {} PrivateLink VPC endpoints for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
