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

pub struct IamControlPlane {
    aws_service: Arc<AwsService>,
}

impl IamControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Sync IAM Users from AWS
    pub async fn sync_users(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        let iam_client = self.aws_service.create_iam_client(aws_account_dto).await?;

        debug!("Fetching IAM Users from AWS");

        let mut all_users = Vec::new();
        let mut n_users = 0;
        let mut list_users_stream = iam_client.list_users().into_paginator().send();

        while let Some(result) = list_users_stream.next().await {
            match result {
                Ok(response) => {
                    for user in response.users() {
                        let arn = user.arn();
                        let user_id = user.user_id();
                        let user_name = user.user_name();

                        let tags = user.tags();
                        let mut tags_json = serde_json::Map::new();
                        for tag in tags {
                            tags_json.insert(
                                tag.key().to_string(),
                                json!(tag.value()),
                            );
                        }

                        let resource_data = json!({
                            "user_name": user_name,
                            "user_id": user_id,
                            "path": user.path(),
                            "create_date": user.create_date().to_string(),
                            "password_last_used": user.password_last_used().map(|d| d.to_string()),
                            "permissions_boundary": user.permissions_boundary().map(|p| json!({
                                "permissions_boundary_type": p.permissions_boundary_type().map(|t| t.as_str().to_string()),
                                "permissions_boundary_arn": p.permissions_boundary_arn(),
                            })),
                        });

                        let resource_dto = AwsResourceDto {
                            id: None,
                            sync_id: Some(sync_id),
                            account_id: aws_account_dto.account_id.clone(),
                            profile: aws_account_dto.profile.clone(),
                            region: "global".to_string(),
                            resource_type: AwsResourceType::IamUser.to_string(),
                            resource_id: user_id.to_string(),
                            arn: arn.to_string(),
                            name: Some(user_name.to_string()),
                            tags: json!(tags_json),
                            resource_data,
                        };

                        all_users.push(resource_dto.into());
                        n_users += 1;
                    }
                }
                Err(e) => {
                    error!("Error fetching IAM users: {:?}", e);
                    return Err(AppError::CloudProvider(format!("Failed to fetch IAM users: {}", e)));
                }
            }
        }

        debug!("Fetched {} IAM users", n_users);
        Ok(all_users)
    }

    /// Sync IAM Roles from AWS
    pub async fn sync_roles(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        let iam_client = self.aws_service.create_iam_client(aws_account_dto).await?;

        debug!("Fetching IAM Roles from AWS");

        let mut all_roles = Vec::new();
        let mut n_roles = 0;
        let mut list_roles_stream = iam_client.list_roles().into_paginator().send();

        while let Some(result) = list_roles_stream.next().await {
            match result {
                Ok(response) => {
                    for role in response.roles() {
                        let arn = role.arn();
                        let role_id = role.role_id();
                        let role_name = role.role_name();

                        let tags = role.tags();
                        let mut tags_json = serde_json::Map::new();
                        for tag in tags {
                            tags_json.insert(
                                tag.key().to_string(),
                                json!(tag.value()),
                            );
                        }

                        let resource_data = json!({
                            "role_name": role_name,
                            "role_id": role_id,
                            "path": role.path(),
                            "create_date": role.create_date().to_string(),
                            "assume_role_policy_document": role.assume_role_policy_document(),
                            "description": role.description(),
                            "max_session_duration": role.max_session_duration(),
                            "permissions_boundary": role.permissions_boundary().map(|p| json!({
                                "permissions_boundary_type": p.permissions_boundary_type().map(|t| t.as_str().to_string()),
                                "permissions_boundary_arn": p.permissions_boundary_arn(),
                            })),
                        });

                        let resource_dto = AwsResourceDto {
                            id: None,
                            sync_id: Some(sync_id),
                            account_id: aws_account_dto.account_id.clone(),
                            profile: aws_account_dto.profile.clone(),
                            region: "global".to_string(),
                            resource_type: AwsResourceType::IamRole.to_string(),
                            resource_id: role_id.to_string(),
                            arn: arn.to_string(),
                            name: Some(role_name.to_string()),
                            tags: json!(tags_json),
                            resource_data,
                        };

                        all_roles.push(resource_dto.into());
                        n_roles += 1;
                    }
                }
                Err(e) => {
                    error!("Error fetching IAM roles: {:?}", e);
                    return Err(AppError::CloudProvider(format!("Failed to fetch IAM roles: {}", e)));
                }
            }
        }

        debug!("Fetched {} IAM roles", n_roles);
        Ok(all_roles)
    }

    /// Sync IAM Policies from AWS
    pub async fn sync_policies(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        let iam_client = self.aws_service.create_iam_client(aws_account_dto).await?;

        debug!("Fetching IAM Policies from AWS");

        let mut all_policies = Vec::new();
        let mut n_policies = 0;
        let mut list_policies_stream = iam_client.list_policies().into_paginator().send();

        while let Some(result) = list_policies_stream.next().await {
            match result {
                Ok(response) => {
                    for policy in response.policies() {
                        let arn = policy.arn().unwrap_or("");
                        let policy_id = policy.policy_id().unwrap_or("");
                        let policy_name = policy.policy_name().unwrap_or("");

                        let tags = policy.tags();
                        let mut tags_json = serde_json::Map::new();
                        for tag in tags {
                            tags_json.insert(
                                tag.key().to_string(),
                                json!(tag.value()),
                            );
                        }

                        let resource_data = json!({
                            "policy_name": policy_name,
                            "policy_id": policy_id,
                            "path": policy.path(),
                            "default_version_id": policy.default_version_id(),
                            "attachment_count": policy.attachment_count(),
                            "permissions_boundary_usage_count": policy.permissions_boundary_usage_count(),
                            "is_attachable": policy.is_attachable(),
                            "description": policy.description(),
                            "create_date": policy.create_date().map(|d| d.to_string()),
                            "update_date": policy.update_date().map(|d| d.to_string()),
                        });

                        let resource_dto = AwsResourceDto {
                            id: None,
                            sync_id: Some(sync_id),
                            account_id: aws_account_dto.account_id.clone(),
                            profile: aws_account_dto.profile.clone(),
                            region: "global".to_string(),
                            resource_type: AwsResourceType::IamPolicy.to_string(),
                            resource_id: policy_id.to_string(),
                            arn: arn.to_string(),
                            name: Some(policy_name.to_string()),
                            tags: json!(tags_json),
                            resource_data,
                        };

                        all_policies.push(resource_dto.into());
                        n_policies += 1;
                    }
                }
                Err(e) => {
                    error!("Error fetching IAM policies: {:?}", e);
                    return Err(AppError::CloudProvider(format!("Failed to fetch IAM policies: {}", e)));
                }
            }
        }

        debug!("Fetched {} IAM policies", n_policies);
        Ok(all_policies)
    }

    /// Sync IAM Groups from AWS
    pub async fn sync_groups(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        let iam_client = self.aws_service.create_iam_client(aws_account_dto).await?;

        debug!("Fetching IAM Groups from AWS");

        let mut all_groups = Vec::new();
        let mut n_groups = 0;
        let mut list_groups_stream = iam_client.list_groups().into_paginator().send();

        while let Some(result) = list_groups_stream.next().await {
            match result {
                Ok(response) => {
                    for group in response.groups() {
                        let arn = group.arn();
                        let group_id = group.group_id();
                        let group_name = group.group_name();

                        let resource_data = json!({
                            "group_name": group_name,
                            "group_id": group_id,
                            "path": group.path(),
                            "create_date": group.create_date().to_string(),
                        });

                        let resource_dto = AwsResourceDto {
                            id: None,
                            sync_id: Some(sync_id),
                            account_id: aws_account_dto.account_id.clone(),
                            profile: aws_account_dto.profile.clone(),
                            region: "global".to_string(),
                            resource_type: AwsResourceType::IamGroup.to_string(),
                            resource_id: group_id.to_string(),
                            arn: arn.to_string(),
                            name: Some(group_name.to_string()),
                            tags: json!({}), // IAM groups don't have tags via list_groups API directly
                            resource_data,
                        };

                        all_groups.push(resource_dto.into());
                        n_groups += 1;
                    }
                }
                Err(e) => {
                    error!("Error fetching IAM groups: {:?}", e);
                    return Err(AppError::CloudProvider(format!("Failed to fetch IAM groups: {}", e)));
                }
            }
        }

        debug!("Fetched {} IAM groups", n_groups);
        Ok(all_groups)
    }
}
