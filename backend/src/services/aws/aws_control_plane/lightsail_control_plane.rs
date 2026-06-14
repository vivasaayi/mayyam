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

pub struct LightsailControlPlane {
    aws_service: Arc<AwsService>,
}

impl LightsailControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Lightsail resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_lightsail_client(aws_account_dto)
            .await?;
        let mut resources = Vec::new();

        resources.extend(sync_instances(&client, aws_account_dto, sync_id).await?);
        resources.extend(sync_static_ips(&client, aws_account_dto, sync_id).await?);

        debug!(
            "Successfully synced {} Lightsail resources for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn sync_instances(
    client: &aws_sdk_lightsail::Client,
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
) -> Result<Vec<AwsResourceModel>, AppError> {
    let mut resources = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut request = client.get_instances();
        if let Some(token) = page_token {
            request = request.page_token(token);
        }

        let response = request.send().await.map_err(|e| {
            error!("Failed to list Lightsail instances: {}", e);
            AppError::ExternalService(format!("Failed to list Lightsail instances: {}", e))
        })?;

        for instance in response.instances() {
            let Some(name) = instance.name() else {
                continue;
            };
            let arn = instance.arn().unwrap_or("").to_string();
            if arn.is_empty() {
                continue;
            }

            let mut resource_data = serde_json::Map::new();
            resource_data.insert("resource_kind".to_string(), json!("instance"));
            resource_data.insert("instance_name".to_string(), json!(name));
            if let Some(state) = instance.state().and_then(|s| s.name()) {
                resource_data.insert("state".to_string(), json!(state));
            }
            if let Some(bundle_id) = instance.bundle_id() {
                resource_data.insert("bundle_id".to_string(), json!(bundle_id));
            }
            if let Some(blueprint_id) = instance.blueprint_id() {
                resource_data.insert("blueprint_id".to_string(), json!(blueprint_id));
            }
            if let Some(public_ip) = instance.public_ip_address() {
                resource_data.insert("public_ip_address".to_string(), json!(public_ip));
            }
            if let Some(private_ip) = instance.private_ip_address() {
                resource_data.insert("private_ip_address".to_string(), json!(private_ip));
            }
            if let Some(is_static_ip) = instance.is_static_ip() {
                resource_data.insert("uses_static_ip".to_string(), json!(is_static_ip));
            }
            if let Some(created_at) = fmt_date(instance.created_at()) {
                resource_data.insert("created_at".to_string(), json!(created_at));
            }
            if let Some(hardware) = instance.hardware() {
                if let Some(cpu_count) = hardware.cpu_count() {
                    resource_data.insert("cpu_count".to_string(), json!(cpu_count));
                }
                if let Some(ram) = hardware.ram_size_in_gb() {
                    resource_data.insert("ram_size_gb".to_string(), json!(ram));
                }
                resource_data.insert("disk_count".to_string(), json!(hardware.disks().len()));
            }

            let public_admin_ports = public_admin_ports(instance);
            resource_data.insert("public_admin_ports".to_string(), json!(public_admin_ports));

            let dto = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: AwsResourceType::LightsailResource.to_string(),
                resource_id: format!("instance/{}", name),
                arn,
                name: Some(name.to_string()),
                tags: lightsail_tags(instance.tags()),
                resource_data: serde_json::Value::Object(resource_data),
            };
            resources.push(dto.into());
        }

        page_token = response.next_page_token().map(String::from);
        if page_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn sync_static_ips(
    client: &aws_sdk_lightsail::Client,
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
) -> Result<Vec<AwsResourceModel>, AppError> {
    let mut resources = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut request = client.get_static_ips();
        if let Some(token) = page_token {
            request = request.page_token(token);
        }

        let response = request.send().await.map_err(|e| {
            error!("Failed to list Lightsail static IPs: {}", e);
            AppError::ExternalService(format!("Failed to list Lightsail static IPs: {}", e))
        })?;

        for static_ip in response.static_ips() {
            let Some(name) = static_ip.name() else {
                continue;
            };
            let arn = static_ip.arn().unwrap_or("").to_string();
            if arn.is_empty() {
                continue;
            }

            let mut resource_data = serde_json::Map::new();
            resource_data.insert("resource_kind".to_string(), json!("static_ip"));
            resource_data.insert("static_ip_name".to_string(), json!(name));
            if let Some(ip_address) = static_ip.ip_address() {
                resource_data.insert("ip_address".to_string(), json!(ip_address));
            }
            if let Some(is_attached) = static_ip.is_attached() {
                resource_data.insert("is_attached".to_string(), json!(is_attached));
            }
            if let Some(attached_to) = static_ip.attached_to() {
                resource_data.insert("attached_to".to_string(), json!(attached_to));
            }
            if let Some(created_at) = fmt_date(static_ip.created_at()) {
                resource_data.insert("created_at".to_string(), json!(created_at));
            }

            let dto = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: AwsResourceType::LightsailResource.to_string(),
                resource_id: format!("static-ip/{}", name),
                arn,
                name: Some(name.to_string()),
                tags: json!({}),
                resource_data: serde_json::Value::Object(resource_data),
            };
            resources.push(dto.into());
        }

        page_token = response.next_page_token().map(String::from);
        if page_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

fn public_admin_ports(instance: &aws_sdk_lightsail::types::Instance) -> Vec<i32> {
    let mut ports = Vec::new();

    if let Some(networking) = instance.networking() {
        for port in networking.ports() {
            let from_port = port.from_port();
            let to_port = port.to_port();
            let access = port.access_from().unwrap_or_default().to_ascii_lowercase();
            let public = access.is_empty() || access.contains("0.0.0.0") || access.contains("::/0");
            if public {
                for admin_port in [22, 3389] {
                    if from_port <= admin_port && admin_port <= to_port {
                        ports.push(admin_port);
                    }
                }
            }
        }
    }

    ports.sort_unstable();
    ports.dedup();
    ports
}

fn lightsail_tags(tags: &[aws_sdk_lightsail::types::Tag]) -> serde_json::Value {
    let mut tags_map = serde_json::Map::new();

    for tag in tags {
        if let Some(key) = tag.key() {
            tags_map.insert(key.to_string(), json!(tag.value().unwrap_or_default()));
        }
    }

    serde_json::Value::Object(tags_map)
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.map(|d| {
        d.fmt(Format::DateTime)
            .unwrap_or_else(|_| format!("{:?}", d))
    })
}
