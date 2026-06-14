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

pub struct GlobalAcceleratorControlPlane {
    aws_service: Arc<AwsService>,
}

impl GlobalAcceleratorControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_accelerators(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Global Accelerators for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_globalaccelerator_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_accelerators();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Global Accelerators: {}", e);
                AppError::ExternalService(format!("Failed to list Global Accelerators: {}", e))
            })?;

            for accelerator in response.accelerators() {
                let arn = match accelerator.accelerator_arn() {
                    Some(a) if !a.is_empty() => a.to_string(),
                    _ => {
                        debug!("Skipping Global Accelerator list entry without an ARN");
                        continue;
                    }
                };

                let name = accelerator
                    .name()
                    .filter(|n| !n.is_empty())
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| arn.clone());

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("accelerator_arn".to_string(), json!(arn));
                resource_data.insert("name".to_string(), json!(name));

                if let Some(ip_address_type) = accelerator.ip_address_type() {
                    resource_data.insert(
                        "ip_address_type".to_string(),
                        json!(ip_address_type.as_str()),
                    );
                }

                if let Some(enabled) = accelerator.enabled() {
                    resource_data.insert("enabled".to_string(), json!(enabled));
                }

                if let Some(status) = accelerator.status() {
                    // DEPLOYED or IN_PROGRESS
                    resource_data.insert("status".to_string(), json!(status.as_str()));
                }

                if let Some(dns_name) = accelerator.dns_name() {
                    resource_data.insert("dns_name".to_string(), json!(dns_name));
                }

                if let Some(dual_stack_dns_name) = accelerator.dual_stack_dns_name() {
                    resource_data.insert(
                        "dual_stack_dns_name".to_string(),
                        json!(dual_stack_dns_name),
                    );
                }

                if let Some(created_time) = accelerator.created_time() {
                    let formatted = created_time
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", created_time));
                    resource_data.insert("created_time".to_string(), json!(formatted));
                }

                if let Some(last_modified_time) = accelerator.last_modified_time() {
                    let formatted = last_modified_time
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", last_modified_time));
                    resource_data.insert("last_modified_time".to_string(), json!(formatted));
                }

                let ip_sets: Vec<serde_json::Value> = accelerator
                    .ip_sets()
                    .iter()
                    .map(|ip_set| {
                        json!({
                            "ip_family": ip_set.ip_family(),
                            "ip_address_count": ip_set.ip_addresses().len(),
                        })
                    })
                    .collect();
                resource_data.insert("ip_sets".to_string(), json!(ip_sets));

                // Enrichment: flow log attributes. A failure here must not fail
                // the sync; the evaluator treats attributes_collected=false as
                // a data gap.
                match client
                    .describe_accelerator_attributes()
                    .accelerator_arn(&arn)
                    .send()
                    .await
                {
                    Ok(attrs_response) => match attrs_response.accelerator_attributes() {
                        Some(attrs) => {
                            resource_data.insert("attributes_collected".to_string(), json!(true));
                            if let Some(flow_logs_enabled) = attrs.flow_logs_enabled() {
                                resource_data.insert(
                                    "flow_logs_enabled".to_string(),
                                    json!(flow_logs_enabled),
                                );
                            }
                            if let Some(bucket) = attrs.flow_logs_s3_bucket() {
                                resource_data
                                    .insert("flow_logs_s3_bucket".to_string(), json!(bucket));
                            }
                            if let Some(prefix) = attrs.flow_logs_s3_prefix() {
                                resource_data
                                    .insert("flow_logs_s3_prefix".to_string(), json!(prefix));
                            }
                        }
                        None => {
                            debug!(
                                "DescribeAcceleratorAttributes returned no attributes for {}",
                                arn
                            );
                            resource_data.insert("attributes_collected".to_string(), json!(false));
                        }
                    },
                    Err(e) => {
                        debug!(
                            "Failed to describe attributes for Global Accelerator {}: {}",
                            arn, e
                        );
                        resource_data.insert("attributes_collected".to_string(), json!(false));
                    }
                }

                // Enrichment: listeners. Paginate for an accurate count, but
                // persist port/protocol detail only from the first page.
                let mut listener_count = 0usize;
                let mut listener_details: Vec<serde_json::Value> = Vec::new();
                let mut listeners_collected = true;
                let mut listener_token: Option<String> = None;
                let mut first_page = true;

                loop {
                    let mut listener_request = client.list_listeners().accelerator_arn(&arn);
                    if let Some(token) = listener_token {
                        listener_request = listener_request.next_token(token);
                    }

                    match listener_request.send().await {
                        Ok(listener_response) => {
                            let listeners = listener_response.listeners();
                            listener_count += listeners.len();
                            if first_page {
                                for listener in listeners {
                                    let port_ranges: Vec<serde_json::Value> = listener
                                        .port_ranges()
                                        .iter()
                                        .map(|pr| {
                                            json!({
                                                "from_port": pr.from_port(),
                                                "to_port": pr.to_port(),
                                            })
                                        })
                                        .collect();
                                    listener_details.push(json!({
                                        "listener_arn": listener.listener_arn(),
                                        "protocol": listener.protocol().map(|p| p.as_str()),
                                        "client_affinity": listener
                                            .client_affinity()
                                            .map(|c| c.as_str()),
                                        "port_ranges": port_ranges,
                                    }));
                                }
                                first_page = false;
                            }
                            listener_token = listener_response.next_token().map(String::from);
                            if listener_token.is_none() {
                                break;
                            }
                        }
                        Err(e) => {
                            debug!(
                                "Failed to list listeners for Global Accelerator {}: {}",
                                arn, e
                            );
                            listeners_collected = false;
                            break;
                        }
                    }
                }

                resource_data.insert(
                    "listeners_collected".to_string(),
                    json!(listeners_collected),
                );
                if listeners_collected {
                    resource_data.insert("listener_count".to_string(), json!(listener_count));
                    resource_data.insert("listeners".to_string(), json!(listener_details));
                }

                let tags = match client
                    .list_tags_for_resource()
                    .resource_arn(&arn)
                    .send()
                    .await
                {
                    Ok(tags_response) => {
                        let mut tags_map = serde_json::Map::new();
                        for tag in tags_response.tags() {
                            tags_map.insert(tag.key().to_string(), json!(tag.value()));
                        }
                        serde_json::Value::Object(tags_map)
                    }
                    Err(e) => {
                        debug!("Failed to list tags for Global Accelerator {}: {}", arn, e);
                        json!({})
                    }
                };

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::GlobalAccelerator.to_string(),
                    resource_id: arn.clone(),
                    arn,
                    name: Some(name),
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
            "Successfully synced {} Global Accelerators for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
