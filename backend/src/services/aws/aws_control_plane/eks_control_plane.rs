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
use tracing::debug;
use uuid::Uuid;

pub struct EksControlPlane {
    aws_service: Arc<AwsService>,
}

impl EksControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_clusters(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EKS clusters for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_eks_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut marker = None;
        loop {
            let mut request = client.list_clusters();
            if let Some(m) = marker {
                request = request.next_token(m);
            }

            let response = match request.send().await {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("Failed to list EKS clusters: {}", e);
                    break;
                }
            };

            let clusters = response.clusters();
            if true {
                for cluster_name in clusters {
                    let describe_response = match client.describe_cluster().name(cluster_name).send().await {
                        Ok(res) => res,
                        Err(e) => {
                            tracing::error!("Failed to describe EKS cluster {}: {}", cluster_name, e);
                            continue;
                        }
                    };

                    if let Some(cluster_info) = describe_response.cluster() {
                        let name = cluster_info.name().unwrap_or_default();
                        let arn = cluster_info.arn().unwrap_or_default();

                        let resource_data = serde_json::json!({
                            "ClusterName": name,
                            "ClusterArn": arn,
                            "Version": cluster_info.version(),
                            "Endpoint": cluster_info.endpoint(),
                            "RoleArn": cluster_info.role_arn(),
                            "Status": cluster_info.status().map(|s| s.as_str()),
                            "CreatedAt": cluster_info.created_at().map(|d| d.to_string()),
                        });

                        let dto = AwsResourceDto {
                            id: None,
                            sync_id: Some(sync_id),
                            account_id: aws_account_dto.account_id.clone(),
                            profile: aws_account_dto.profile.clone(),
                            region: aws_account_dto.default_region.clone(),
                            resource_type: AwsResourceType::EksCluster.to_string(),
                            resource_id: name.to_string(),
                            arn: arn.to_string(),
                            name: Some(name.to_string()),
                            tags: json!({}),
                            resource_data,
                        };

                        resources.push(dto.into());
                    }
                }
            }

            marker = response.next_token().map(String::from);
            if marker.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} EKS clusters for account: {} with sync_id: {}",
            resources.len(), &aws_account_dto.account_id, sync_id
        );

        Ok(resources)
    }

    pub async fn sync_fargate_profiles(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EKS Fargate profiles for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_eks_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut cluster_names = Vec::new();
        let mut marker = None;
        loop {
            let mut request = client.list_clusters();
            if let Some(m) = marker {
                request = request.next_token(m);
            }
            if let Ok(res) = request.send().await {
                let clusters = res.clusters();
                if !clusters.is_empty() {
                    cluster_names.extend(clusters.iter().map(String::from));
                }
                marker = res.next_token().map(String::from);
                if marker.is_none() {
                    break;
                }
            } else {
                break;
            }
        }

        for cluster_name in cluster_names {
            let mut fp_marker = None;
            loop {
                let mut request = client.list_fargate_profiles().cluster_name(&cluster_name);
                if let Some(m) = fp_marker {
                    request = request.next_token(m);
                }

                let response = match request.send().await {
                    Ok(res) => res,
                    Err(e) => {
                        tracing::error!("Failed to list Fargate Profiles for cluster {}: {}", cluster_name, e);
                        break;
                    }
                };

                let profiles = response.fargate_profile_names();
                if true {
                    for profile_name in profiles {
                        let describe_response = match client.describe_fargate_profile()
                            .cluster_name(&cluster_name)
                            .fargate_profile_name(profile_name)
                            .send().await {
                            Ok(res) => res,
                            Err(e) => {
                                tracing::error!("Failed to describe Fargate Profile {}: {}", profile_name, e);
                                continue;
                            }
                        };

                        if let Some(profile_info) = describe_response.fargate_profile() {
                            let name = profile_info.fargate_profile_name().unwrap_or_default();
                            let arn = profile_info.fargate_profile_arn().unwrap_or_default();

                            let resource_data = serde_json::json!({
                                "FargateProfileName": name,
                                "FargateProfileArn": arn,
                                "ClusterName": profile_info.cluster_name(),
                                "Status": profile_info.status().map(|s| s.as_str()),
                                "PodExecutionRoleArn": profile_info.pod_execution_role_arn(),
                                "Subnets": profile_info.subnets(),
                            });

                            let dto = AwsResourceDto {
                                id: None,
                                sync_id: Some(sync_id),
                                account_id: aws_account_dto.account_id.clone(),
                                profile: aws_account_dto.profile.clone(),
                                region: aws_account_dto.default_region.clone(),
                                resource_type: AwsResourceType::FargateProfile.to_string(),
                                resource_id: name.to_string(),
                                arn: arn.to_string(),
                                name: Some(name.to_string()),
                                tags: json!({}),
                                resource_data,
                            };

                            resources.push(dto.into());
                        }
                    }
                }

                fp_marker = response.next_token().map(String::from);
                if fp_marker.is_none() {
                    break;
                }
            }
        }

        debug!(
            "Successfully synced {} EKS Fargate Profiles for account: {} with sync_id: {}",
            resources.len(), &aws_account_dto.account_id, sync_id
        );

        Ok(resources)
    }
}
