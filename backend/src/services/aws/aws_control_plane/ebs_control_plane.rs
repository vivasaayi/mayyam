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
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel, AwsResourceType};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::aws::service::AwsService;
use aws_sdk_ec2::types::{Volume, Snapshot};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct EbsControlPlane {
    aws_service: Arc<AwsService>,
}

impl EbsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Sync EBS Volumes
    pub async fn sync_volumes(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EBS volumes for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // Get all volumes
        let mut next_token = None;
        loop {
            let mut request = client.describe_volumes();
            if let Some(token) = &next_token {
                request = request.next_token(token);
            }

            match request.send().await {
                Ok(response) => {
                    if let Some(volumes) = &response.volumes {
                        for volume in volumes {
                            match self.create_volume_resource(volume, aws_account_dto, sync_id).await {
                                Ok(resource) => all_resources.push(resource),
                                Err(e) => error!("Failed to create volume resource: {}", e),
                            }
                        }
                    }

                    next_token = response.next_token;
                    if next_token.is_none() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to get volumes: {}", e);
                    return Err(AppError::CloudProvider(e.to_string()));
                }
            }
        }

        info!("Synced {} EBS volumes", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Sync EBS Snapshots
    pub async fn sync_snapshots(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EBS snapshots for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // Get all snapshots owned by this account
        let mut next_token = None;
        loop {
            let mut request = client.describe_snapshots().owner_ids("self");
            if let Some(token) = &next_token {
                request = request.next_token(token);
            }

            match request.send().await {
                Ok(response) => {
                    if let Some(snapshots) = &response.snapshots {
                        for snapshot in snapshots {
                            match self.create_snapshot_resource(snapshot, aws_account_dto, sync_id).await {
                                Ok(resource) => all_resources.push(resource),
                                Err(e) => error!("Failed to create snapshot resource: {}", e),
                            }
                        }
                    }

                    next_token = response.next_token;
                    if next_token.is_none() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to get snapshots: {}", e);
                    return Err(AppError::CloudProvider(e.to_string()));
                }
            }
        }

        info!("Synced {} EBS snapshots", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Create EBS volume resource from AWS SDK model
    async fn create_volume_resource(
        &self,
        volume: &Volume,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let resource_id = volume.volume_id.as_ref()
            .ok_or_else(|| AppError::Validation("EBS volume ID missing".to_string()))?;

        let arn = format!(
            "arn:aws:ec2:{}:{}:volume/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, resource_id
        );

        let mut tags_map = serde_json::Map::new();
        if let Some(tags) = &volume.tags {
            for tag in tags {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    tags_map.insert(key.to_string(), serde_json::Value::String(value.to_string()));
                }
            }
        }

        let resource_data = serde_json::json!({
            "volume_type": volume.volume_type().map(|vt| vt.as_str()),
            "size": volume.size,
            "iops": volume.iops,
            "throughput": volume.throughput,
            "availability_zone": volume.availability_zone,
            "state": volume.state().map(|s| s.as_str()),
            "encrypted": volume.encrypted,
            "kms_key_id": volume.kms_key_id,
            "multi_attach_enabled": volume.multi_attach_enabled,
            "fast_restored": volume.fast_restored,
            "create_time": volume.create_time.map(|dt| dt.to_string())
        });

        Ok(AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::EbsVolume.to_string(),
            resource_id: resource_id.to_string(),
            arn,
            name: volume.tags.as_ref().and_then(|tags| {
                tags.iter().find(|tag| tag.key() == Some("Name"))
                    .and_then(|tag| tag.value())
                    .map(|s| s.to_string())
            }),
            tags: serde_json::Value::Object(tags_map),
            resource_data,
        })
    }

    /// Create EBS snapshot resource from AWS SDK model
    async fn create_snapshot_resource(
        &self,
        snapshot: &Snapshot,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let resource_id = snapshot.snapshot_id.as_ref()
            .ok_or_else(|| AppError::Validation("EBS snapshot ID missing".to_string()))?;

        let arn = format!(
            "arn:aws:ec2:{}:{}:snapshot/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, resource_id
        );

        let mut tags_map = serde_json::Map::new();
        if let Some(tags) = &snapshot.tags {
            for tag in tags {
                if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    tags_map.insert(key.to_string(), serde_json::Value::String(value.to_string()));
                }
            }
        }

        let resource_data = serde_json::json!({
            "volume_id": snapshot.volume_id,
            "volume_size": snapshot.volume_size,
            "description": snapshot.description,
            "state": snapshot.state().map(|s| s.as_str()),
            "progress": snapshot.progress,
            "encrypted": snapshot.encrypted,
            "kms_key_id": snapshot.kms_key_id,
            "owner_id": snapshot.owner_id,
            "start_time": snapshot.start_time.map(|dt| dt.to_string()),
            "completion_time": snapshot.completion_time.map(|dt| dt.to_string())
        });

        Ok(AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::EbsSnapshot.to_string(),
            resource_id: resource_id.to_string(),
            arn,
            name: snapshot.tags.as_ref().and_then(|tags| {
                tags.iter().find(|tag| tag.key() == Some("Name"))
                    .and_then(|tag| tag.value())
                    .map(|s| s.to_string())
            }),
            tags: serde_json::Value::Object(tags_map),
            resource_data,
        })
    }
}