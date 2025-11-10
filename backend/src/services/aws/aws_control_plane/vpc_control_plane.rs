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


use aws_sdk_ec2::Client as Ec2Client;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;

// Control plane implementation for VPC resources
pub struct VpcControlPlane {
    aws_service: Arc<AwsService>,
}

impl VpcControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Sync VPCs from AWS
    pub async fn sync_vpcs(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!("Syncing VPCs with sync_id: {}.", sync_id);
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let response = client.describe_vpcs().send().await.map_err(|e| {
            error!("Failed to describe VPCs: {}", &e);
            AppError::ExternalService(format!("Failed to describe VPCs: {}", e))
        })?;

        let mut vpcs = Vec::new();

        for vpc in response.vpcs() {
            let vpc_id = vpc.vpc_id().unwrap_or_default().to_string();
            let arn = format!(
                "arn:aws:ec2:{}:{}:vpc/{}",
                aws_account_dto.default_region, aws_account_dto.account_id, vpc_id
            );

            // Extract tags and name
            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            for tag in vpc.tags() {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }

            // Build resource data
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("vpc_id".to_string(), json!(vpc_id));

            if let Some(cidr_block) = vpc.cidr_block() {
                resource_data.insert("cidr_block".to_string(), json!(cidr_block));
            }

            if let Some(state) = vpc.state().map(|s| s.as_str()) {
                resource_data.insert("state".to_string(), json!(state));
            }

            if let Some(is_default) = vpc.is_default() {
                resource_data.insert("is_default".to_string(), json!(is_default));
            }

            // Create resource DTO
            let vpc_resource = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone().to_string(),
                resource_type: "Vpc".to_string(),
                resource_id: vpc_id.clone(),
                arn: arn.clone(),
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };

            vpcs.push(vpc_resource);
        }

        Ok(vpcs.into_iter().map(|v| v.into()).collect())
    }

    /// Sync Subnets from AWS
    pub async fn sync_subnets(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!("Syncing Subnets with sync_id: {}.", sync_id);
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let response = client.describe_subnets().send().await.map_err(|e| {
            error!("Failed to describe subnets: {}", &e);
            AppError::ExternalService(format!("Failed to describe subnets: {}", e))
        })?;

        let mut subnets = Vec::new();

        for subnet in response.subnets() {
            let subnet_id = subnet.subnet_id().unwrap_or_default().to_string();
            let arn = format!(
                "arn:aws:ec2:{}:{}:subnet/{}",
                aws_account_dto.default_region, aws_account_dto.account_id, subnet_id
            );

            // Extract tags and name
            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            for tag in subnet.tags() {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }

            // Build resource data
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("subnet_id".to_string(), json!(subnet_id));

            if let Some(cidr_block) = subnet.cidr_block() {
                resource_data.insert("cidr_block".to_string(), json!(cidr_block));
            }

            if let Some(vpc_id) = subnet.vpc_id() {
                resource_data.insert("vpc_id".to_string(), json!(vpc_id));
            }

            if let Some(az) = subnet.availability_zone() {
                resource_data.insert("availability_zone".to_string(), json!(az));
            }

            if let Some(az_id) = subnet.availability_zone_id() {
                resource_data.insert("availability_zone_id".to_string(), json!(az_id));
            }

            if let Some(state) = subnet.state().map(|s| s.as_str()) {
                resource_data.insert("state".to_string(), json!(state));
            }

            // Create resource DTO
            let subnet_resource = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone().to_string(),
                resource_type: "Subnet".to_string(),
                resource_id: subnet_id.clone(),
                arn: arn.clone(),
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };

            subnets.push(subnet_resource);
        }

        Ok(subnets.into_iter().map(|s| s.into()).collect())
    }

    /// Sync Security Groups from AWS
    pub async fn sync_security_groups(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!("Syncing Security Groups with sync_id: {}.", sync_id);
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let response = client.describe_security_groups().send().await.map_err(|e| {
            error!("Failed to describe security groups: {}", &e);
            AppError::ExternalService(format!("Failed to describe security groups: {}", e))
        })?;

        let mut security_groups = Vec::new();

        for sg in response.security_groups() {
            let group_id = sg.group_id().unwrap_or_default().to_string();
            let arn = format!(
                "arn:aws:ec2:{}:{}:security-group/{}",
                aws_account_dto.default_region, aws_account_dto.account_id, group_id
            );

            // Extract tags and name
            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            for tag in sg.tags() {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }

            // Build resource data
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("group_id".to_string(), json!(group_id));

            if let Some(group_name) = sg.group_name() {
                resource_data.insert("group_name".to_string(), json!(group_name));
                if name.is_none() {
                    name = Some(group_name.to_string());
                }
            }

            if let Some(description) = sg.description() {
                resource_data.insert("description".to_string(), json!(description));
            }

            if let Some(vpc_id) = sg.vpc_id() {
                resource_data.insert("vpc_id".to_string(), json!(vpc_id));
            }

            // Create resource DTO
            let sg_resource = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone().to_string(),
                resource_type: "SecurityGroup".to_string(),
                resource_id: group_id.clone(),
                arn: arn.clone(),
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };

            security_groups.push(sg_resource);
        }

        Ok(security_groups.into_iter().map(|sg| sg.into()).collect())
    }

    /// Sync Internet Gateways from AWS
    pub async fn sync_internet_gateways(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!("Syncing Internet Gateways with sync_id: {}.", sync_id);
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let response = client.describe_internet_gateways().send().await.map_err(|e| {
            error!("Failed to describe internet gateways: {}", &e);
            AppError::ExternalService(format!("Failed to describe internet gateways: {}", e))
        })?;

        let mut internet_gateways = Vec::new();

        for igw in response.internet_gateways() {
            let igw_id = igw.internet_gateway_id().unwrap_or_default().to_string();
            let arn = format!(
                "arn:aws:ec2:{}:{}:internet-gateway/{}",
                aws_account_dto.default_region, aws_account_dto.account_id, igw_id
            );

            // Extract tags and name
            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            for tag in igw.tags() {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }

            // Build resource data
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("internet_gateway_id".to_string(), json!(igw_id));

            // Create resource DTO
            let igw_resource = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone().to_string(),
                resource_type: "InternetGateway".to_string(),
                resource_id: igw_id.clone(),
                arn: arn.clone(),
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };

            internet_gateways.push(igw_resource);
        }

        Ok(internet_gateways.into_iter().map(|igw| igw.into()).collect())
    }

    /// Sync NAT Gateways from AWS
    pub async fn sync_nat_gateways(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!("Syncing NAT Gateways with sync_id: {}.", sync_id);
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let response = client.describe_nat_gateways().send().await.map_err(|e| {
            error!("Failed to describe NAT gateways: {}", &e);
            AppError::ExternalService(format!("Failed to describe NAT gateways: {}", e))
        })?;

        let mut nat_gateways = Vec::new();

        for nat_gw in response.nat_gateways() {
            let nat_gw_id = nat_gw.nat_gateway_id().unwrap_or_default().to_string();
            let arn = format!(
                "arn:aws:ec2:{}:{}:natgateway/{}",
                aws_account_dto.default_region, aws_account_dto.account_id, nat_gw_id
            );

            // Extract tags and name
            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            for tag in nat_gw.tags() {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }

            // Build resource data
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("nat_gateway_id".to_string(), json!(nat_gw_id));

            if let Some(state) = nat_gw.state().map(|s| s.as_str()) {
                resource_data.insert("state".to_string(), json!(state));
            }

            if let Some(subnet_id) = nat_gw.subnet_id() {
                resource_data.insert("subnet_id".to_string(), json!(subnet_id));
            }

            if let Some(vpc_id) = nat_gw.vpc_id() {
                resource_data.insert("vpc_id".to_string(), json!(vpc_id));
            }

            // Create resource DTO
            let nat_gw_resource = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone().to_string(),
                resource_type: "NatGateway".to_string(),
                resource_id: nat_gw_id.clone(),
                arn: arn.clone(),
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };

            nat_gateways.push(nat_gw_resource);
        }

        Ok(nat_gateways.into_iter().map(|ngw| ngw.into()).collect())
    }

    /// Sync Route Tables from AWS
    pub async fn sync_route_tables(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!("Syncing Route Tables with sync_id: {}.", sync_id);
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let response = client.describe_route_tables().send().await.map_err(|e| {
            error!("Failed to describe route tables: {}", &e);
            AppError::ExternalService(format!("Failed to describe route tables: {}", e))
        })?;

        let mut route_tables = Vec::new();

        for rt in response.route_tables() {
            let rt_id = rt.route_table_id().unwrap_or_default().to_string();
            let arn = format!(
                "arn:aws:ec2:{}:{}:route-table/{}",
                aws_account_dto.default_region, aws_account_dto.account_id, rt_id
            );

            // Extract tags and name
            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            for tag in rt.tags() {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }

            // Build resource data
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("route_table_id".to_string(), json!(rt_id));

            if let Some(vpc_id) = rt.vpc_id() {
                resource_data.insert("vpc_id".to_string(), json!(vpc_id));
            }

            // Create resource DTO
            let rt_resource = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone().to_string(),
                resource_type: "RouteTable".to_string(),
                resource_id: rt_id.clone(),
                arn: arn.clone(),
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };

            route_tables.push(rt_resource);
        }

        Ok(route_tables.into_iter().map(|rt| rt.into()).collect())
    }

    /// Sync Network ACLs from AWS
    pub async fn sync_network_acls(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!("Syncing Network ACLs with sync_id: {}.", sync_id);
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let response = client.describe_network_acls().send().await.map_err(|e| {
            error!("Failed to describe network ACLs: {}", &e);
            AppError::ExternalService(format!("Failed to describe network ACLs: {}", e))
        })?;

        let mut network_acls = Vec::new();

        for nacl in response.network_acls() {
            let nacl_id = nacl.network_acl_id().unwrap_or_default().to_string();
            let arn = format!(
                "arn:aws:ec2:{}:{}:network-acl/{}",
                aws_account_dto.default_region, aws_account_dto.account_id, nacl_id
            );

            // Extract tags and name
            let mut tags_map = serde_json::Map::new();
            let mut name = None;

            for tag in nacl.tags() {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }

            // Build resource data
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("network_acl_id".to_string(), json!(nacl_id));

            if let Some(vpc_id) = nacl.vpc_id() {
                resource_data.insert("vpc_id".to_string(), json!(vpc_id));
            }

            if let Some(is_default) = nacl.is_default() {
                resource_data.insert("is_default".to_string(), json!(is_default));
            }

            // Create resource DTO
            let nacl_resource = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone().to_string(),
                resource_type: "NetworkAcl".to_string(),
                resource_id: nacl_id.clone(),
                arn: arn.clone(),
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };

            network_acls.push(nacl_resource);
        }

        Ok(network_acls.into_iter().map(|nacl| nacl.into()).collect())
    }
}