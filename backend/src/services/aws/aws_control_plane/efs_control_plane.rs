use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel, AwsResourceType};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::aws::service::AwsService;
use aws_sdk_efs::types::FileSystemDescription;
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct EfsControlPlane {
    aws_service: Arc<AwsService>,
}

impl EfsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Sync EFS File Systems
    pub async fn sync_file_systems(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EFS file systems for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_efs_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // Get all file systems
        let mut next_marker = None;
        loop {
            let mut request = client.describe_file_systems();
            if let Some(marker) = &next_marker {
                request = request.marker(marker);
            }

            match request.send().await {
                Ok(response) => {
                    if let Some(file_systems) = &response.file_systems {
                        for fs in file_systems {
                            match self.create_file_system_resource(fs, aws_account_dto, sync_id).await {
                                Ok(resource) => all_resources.push(resource),
                                Err(e) => error!("Failed to create file system resource: {}", e),
                            }
                        }
                    }

                    next_marker = response.next_marker;
                    if next_marker.is_none() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to get file systems: {}", e);
                    return Err(AppError::CloudProvider(e.to_string()));
                }
            }
        }

        info!("Synced {} EFS file systems", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Create EFS file system resource from AWS SDK model
    async fn create_file_system_resource(
        &self,
        fs: &FileSystemDescription,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let resource_id = fs.file_system_id().to_string();

        let arn = format!(
            "arn:aws:elasticfilesystem:{}:{}:file-system/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, resource_id
        );

        let resource_data = serde_json::json!({
            "name": fs.name,
            "creation_time": Some(fs.creation_time().to_string()),
            "life_cycle_state": fs.life_cycle_state.as_str(),
            "performance_mode": fs.performance_mode.as_str(),
            "throughput_mode": fs.throughput_mode().map(|tm| tm.as_str()),
            "provisioned_throughput_in_mibps": fs.provisioned_throughput_in_mibps,
            "encrypted": fs.encrypted,
            "kms_key_id": fs.kms_key_id,
            "size_in_bytes": {
                "value": fs.size_in_bytes.as_ref().map(|sib| sib.value),
                "timestamp": fs.size_in_bytes.as_ref().and_then(|sib| sib.timestamp).map(|dt| dt.to_string()),
                "value_in_ia": fs.size_in_bytes.as_ref().and_then(|sib| sib.value_in_ia),
                "value_in_standard": fs.size_in_bytes.as_ref().and_then(|sib| sib.value_in_standard)
            },
            "number_of_mount_targets": fs.number_of_mount_targets,
            "owner_id": fs.owner_id
        });

        Ok(AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::EfsFileSystem.to_string(),
            resource_id: resource_id.to_string(),
            arn,
            name: fs.name.clone(),
            tags: serde_json::Value::Object(serde_json::Map::new()), // EFS tags are separate, would need additional API call
            resource_data,
        })
    }
}