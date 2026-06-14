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
use aws_sdk_ec2::types::{
    AutoAcceptSharedAttachmentsValue, DefaultRouteTableAssociationValue,
    DefaultRouteTablePropagationValue, DnsSupportValue, MulticastSupportValue, VpnEcmpSupportValue,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct TransitGatewayControlPlane {
    aws_service: Arc<AwsService>,
}

impl TransitGatewayControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_transit_gateways(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Transit Gateways for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        // Build attachment counts per transit gateway first so each gateway
        // row records how many attachments it carries.
        let mut attachment_counts: HashMap<String, usize> = HashMap::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_transit_gateway_attachments();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe Transit Gateway attachments: {}", e);
                AppError::ExternalService(format!(
                    "Failed to describe Transit Gateway attachments: {}",
                    e
                ))
            })?;

            for attachment in response.transit_gateway_attachments() {
                if let Some(tgw_id) = attachment.transit_gateway_id() {
                    *attachment_counts.entry(tgw_id.to_string()).or_insert(0) += 1;
                }
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_transit_gateways();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe Transit Gateways: {}", e);
                AppError::ExternalService(format!("Failed to describe Transit Gateways: {}", e))
            })?;

            for tgw in response.transit_gateways() {
                let tgw_id = match tgw.transit_gateway_id() {
                    Some(id) => id.to_string(),
                    None => {
                        debug!("Skipping Transit Gateway entry without an ID");
                        continue;
                    }
                };

                let arn = tgw.transit_gateway_arn().unwrap_or("").to_string();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("transit_gateway_id".to_string(), json!(tgw_id));
                resource_data.insert("arn".to_string(), json!(arn));

                if let Some(state) = tgw.state() {
                    resource_data.insert("state".to_string(), json!(state.as_str()));
                }

                if let Some(owner_id) = tgw.owner_id() {
                    resource_data.insert("owner_id".to_string(), json!(owner_id));
                }

                if let Some(description) = tgw.description() {
                    resource_data.insert("description".to_string(), json!(description));
                }

                if let Some(creation_time) = tgw.creation_time() {
                    let formatted = creation_time
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", creation_time));
                    resource_data.insert("creation_time".to_string(), json!(formatted));
                }

                if let Some(options) = tgw.options() {
                    if let Some(amazon_side_asn) = options.amazon_side_asn() {
                        resource_data.insert("amazon_side_asn".to_string(), json!(amazon_side_asn));
                    }

                    if let Some(auto_accept) = options.auto_accept_shared_attachments() {
                        resource_data.insert(
                            "auto_accept_shared_attachments".to_string(),
                            json!(*auto_accept == AutoAcceptSharedAttachmentsValue::Enable),
                        );
                    }

                    if let Some(association) = options.default_route_table_association() {
                        resource_data.insert(
                            "default_route_table_association".to_string(),
                            json!(*association == DefaultRouteTableAssociationValue::Enable),
                        );
                    }

                    if let Some(propagation) = options.default_route_table_propagation() {
                        resource_data.insert(
                            "default_route_table_propagation".to_string(),
                            json!(*propagation == DefaultRouteTablePropagationValue::Enable),
                        );
                    }

                    if let Some(dns_support) = options.dns_support() {
                        resource_data.insert(
                            "dns_support".to_string(),
                            json!(*dns_support == DnsSupportValue::Enable),
                        );
                    }

                    if let Some(vpn_ecmp_support) = options.vpn_ecmp_support() {
                        resource_data.insert(
                            "vpn_ecmp_support".to_string(),
                            json!(*vpn_ecmp_support == VpnEcmpSupportValue::Enable),
                        );
                    }

                    if let Some(multicast_support) = options.multicast_support() {
                        resource_data.insert(
                            "multicast_support".to_string(),
                            json!(*multicast_support == MulticastSupportValue::Enable),
                        );
                    }
                }

                resource_data.insert(
                    "attachment_count".to_string(),
                    json!(attachment_counts.get(&tgw_id).copied().unwrap_or(0)),
                );

                let mut tags_map = serde_json::Map::new();
                for tag in tgw.tags() {
                    if let Some(key) = tag.key() {
                        tags_map.insert(key.to_string(), json!(tag.value().unwrap_or("")));
                    }
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::TransitGateway.to_string(),
                    resource_id: tgw_id.clone(),
                    arn,
                    name: Some(tgw_id),
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
            "Successfully synced {} Transit Gateways for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
