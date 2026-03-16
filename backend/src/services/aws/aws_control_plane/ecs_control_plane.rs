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

pub struct EcsControlPlane {
    aws_service: Arc<AwsService>,
}

impl EcsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_clusters(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing ECS clusters for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_ecs_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut marker = None;
        loop {
            let mut request = client.list_clusters();
            if let Some(m) = marker.clone() {
                request = request.next_token(m);
            }

            let response = match request.send().await {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("Failed to list ECS clusters: {}", e);
                    break;
                }
            };

            let cluster_arns = response.cluster_arns();
            if true {
                if !cluster_arns.is_empty() {
                    let describe_response = match client.describe_clusters().set_clusters(Some(cluster_arns.to_vec())).send().await {
                        Ok(res) => res,
                        Err(e) => {
                            tracing::error!("Failed to describe ECS clusters: {}", e);
                            continue;
                        }
                    };

                    let clusters = describe_response.clusters();
                    if true {
                        for cluster in clusters {
                            let name = cluster.cluster_name().unwrap_or_default();
                            let arn = cluster.cluster_arn().unwrap_or_default();

                            let resource_data = serde_json::json!({
                                "ClusterName": name,
                                "ClusterArn": arn,
                                "Status": cluster.status().unwrap_or_default(),
                                "RegisteredContainerInstancesCount": cluster.registered_container_instances_count(),
                                "RunningTasksCount": cluster.running_tasks_count(),
                                "PendingTasksCount": cluster.pending_tasks_count(),
                                "ActiveServicesCount": cluster.active_services_count(),
                            });

                            let dto = AwsResourceDto {
                                id: None,
                                sync_id: Some(sync_id),
                                account_id: aws_account_dto.account_id.clone(),
                                profile: aws_account_dto.profile.clone(),
                                region: aws_account_dto.default_region.clone(),
                                resource_type: AwsResourceType::EcsCluster.to_string(),
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
            }

            let next_marker = response.next_token().map(String::from);
            marker = next_marker;
            if marker.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} ECS clusters for account: {} with sync_id: {}",
            resources.len(), &aws_account_dto.account_id, sync_id
        );

        Ok(resources)
    }

    pub async fn sync_services(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing ECS services for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_ecs_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        // Need to list clusters first
        let mut cluster_arns = Vec::new();
        let mut marker = None;
        loop {
            let mut request = client.list_clusters();
            if let Some(m) = marker.clone() {
                request = request.next_token(m);
            }
            if let Ok(res) = request.send().await {
                let arns = res.cluster_arns();
                if !arns.is_empty() {
                    cluster_arns.extend(arns.iter().map(String::from));
                }
                marker = res.next_token().map(String::from);
                if marker.is_none() { break; }
            } else { break; }
        }

        for cluster_arn in cluster_arns {
            let mut service_marker = None;
            loop {
                let mut request = client.list_services().cluster(&cluster_arn);
                if let Some(m) = service_marker.clone() {
                    request = request.next_token(m);
                }

                let response = match request.send().await {
                    Ok(res) => res,
                    Err(e) => {
                        tracing::error!("Failed to list ECS services for cluster {}: {}", cluster_arn, e);
                        break;
                    }
                };

                let service_arns = response.service_arns();
                if true {
                    if !service_arns.is_empty() {
                        // describe_services max 10 at a time, but for simplicity assume service_arns is small chunks
                        // AWS SDK list_services returns max 10 by default
                        let describe_response = match client.describe_services().cluster(&cluster_arn).set_services(Some(service_arns.to_vec())).send().await {
                            Ok(res) => res,
                            Err(e) => {
                                tracing::error!("Failed to describe ECS services: {}", e);
                                continue;
                            }
                        };

                        let services = describe_response.services();
                        if true {
                            for service in services {
                                let name = service.service_name().unwrap_or_default();
                                let arn = service.service_arn().unwrap_or_default();

                                let resource_data = serde_json::json!({
                                    "ServiceName": name,
                                    "ServiceArn": arn,
                                    "ClusterArn": service.cluster_arn(),
                                    "Status": service.status(),
                                    "DesiredCount": service.desired_count(),
                                    "RunningCount": service.running_count(),
                                    "PendingCount": service.pending_count(),
                                    "LaunchType": service.launch_type().map(|t| t.as_str()),
                                    "RoleArn": service.role_arn(),
                                    "CreatedAt": service.created_at().map(|d| d.to_string()),
                                });

                                let dto = AwsResourceDto {
                                    id: None,
                                    sync_id: Some(sync_id),
                                    account_id: aws_account_dto.account_id.clone(),
                                    profile: aws_account_dto.profile.clone(),
                                    region: aws_account_dto.default_region.clone(),
                                    resource_type: AwsResourceType::EcsService.to_string(),
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
                }

                service_marker = response.next_token().map(String::from);
                if service_marker.is_none() {
                    break;
                }
            }
        }

        debug!(
            "Successfully synced {} ECS services for account: {} with sync_id: {}",
            resources.len(), &aws_account_dto.account_id, sync_id
        );

        Ok(resources)
    }

    pub async fn sync_tasks(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing ECS tasks for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_ecs_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut cluster_arns = Vec::new();
        let mut marker = None;
        loop {
            let mut request = client.list_clusters();
            if let Some(m) = marker.clone() {
                request = request.next_token(m);
            }
            if let Ok(res) = request.send().await {
                let arns = res.cluster_arns();
                if !arns.is_empty() {
                    cluster_arns.extend(arns.iter().map(String::from));
                }
                marker = res.next_token().map(String::from);
                if marker.is_none() { break; }
            } else { break; }
        }

        for cluster_arn in cluster_arns {
            let mut task_marker = None;
            loop {
                // To get all running/pending/stopped tasks, list_tasks without filter
                let mut request = client.list_tasks().cluster(&cluster_arn);
                if let Some(m) = task_marker.clone() {
                    request = request.next_token(m);
                }

                let response = match request.send().await {
                    Ok(res) => res,
                    Err(e) => {
                        tracing::error!("Failed to list ECS tasks for cluster {}: {}", cluster_arn, e);
                        break;
                    }
                };

                let task_arns = response.task_arns();
                if true {
                    if !task_arns.is_empty() {
                        let describe_response = match client.describe_tasks().cluster(&cluster_arn).set_tasks(Some(task_arns.to_vec())).send().await {
                            Ok(res) => res,
                            Err(e) => {
                                tracing::error!("Failed to describe ECS tasks: {}", e);
                                continue;
                            }
                        };

                        let tasks = describe_response.tasks();
                        if true {
                            for task in tasks {
                                let arn = task.task_arn().unwrap_or_default();
                                // Task ID is last part of arn
                                let name = arn.split('/').last().unwrap_or(&arn);

                                let resource_data = serde_json::json!({
                                    "TaskArn": arn,
                                    "ClusterArn": task.cluster_arn(),
                                    "TaskDefinitionArn": task.task_definition_arn(),
                                    "LastStatus": task.last_status().unwrap_or_default(),
                                    "DesiredStatus": task.desired_status().unwrap_or_default(),
                                    "Cpu": task.cpu(),
                                    "Memory": task.memory(),
                                    "LaunchType": task.launch_type().map(|t| t.as_str()),
                                    "CreatedAt": task.created_at().map(|d| d.to_string()),
                                    "Group": task.group(),
                                });

                                let dto = AwsResourceDto {
                                    id: None,
                                    sync_id: Some(sync_id),
                                    account_id: aws_account_dto.account_id.clone(),
                                    profile: aws_account_dto.profile.clone(),
                                    region: aws_account_dto.default_region.clone(),
                                    resource_type: AwsResourceType::EcsTask.to_string(),
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
                }

                task_marker = response.next_token().map(String::from);
                if task_marker.is_none() {
                    break;
                }
            }
        }

        debug!(
            "Successfully synced {} ECS tasks for account: {} with sync_id: {}",
            resources.len(), &aws_account_dto.account_id, sync_id
        );

        Ok(resources)
    }
}
