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
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::aws::service::AwsService;
use crate::utils::time_conversion::AwsDateTimeExt;
use aws_sdk_apigateway::types::{RestApi, Stage, Resource, Method};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiGatewayControlPlane {
    aws_service: Arc<AwsService>,
}

impl ApiGatewayControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Sync API Gateway REST APIs
    pub async fn sync_rest_apis(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing API Gateway REST APIs for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_api_gateway_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // Get all REST APIs
        match client.get_rest_apis().send().await {
            Ok(response) => {
                if let Some(items) = response.items {
                    for api in items {
                        match self.create_rest_api_resource(&api, aws_account_dto, sync_id).await {
                            Ok(resource) => all_resources.push(resource),
                            Err(e) => error!("Failed to create REST API resource: {}", e),
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to get REST APIs: {}", e);
            }
        }

        info!("Synced {} REST APIs", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Sync API Gateway Stages
    pub async fn sync_stages(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing API Gateway stages for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_api_gateway_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // First get all REST APIs to iterate through their stages
        match client.get_rest_apis().send().await {
            Ok(response) => {
                if let Some(apis) = response.items {
                    for api in apis {
                        if let Some(api_id) = &api.id {
                            // Get stages for this API
                            match client.get_stages().rest_api_id(api_id).send().await {
                                Ok(stages_response) => {
                                    if let Some(items) = stages_response.item {
                                        for stage in items {
                                            match self.create_stage_resource(&stage, &api, aws_account_dto, sync_id).await {
                                                Ok(resource) => all_resources.push(resource),
                                                Err(e) => error!("Failed to create stage resource: {}", e),
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to get stages for API {}: {}", api_id, e);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to get REST APIs for stages: {}", e);
            }
        }

        info!("Synced {} stages", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Sync API Gateway Resources
    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing API Gateway resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_api_gateway_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // First get all REST APIs
        match client.get_rest_apis().send().await {
            Ok(response) => {
                if let Some(apis) = response.items {
                    for api in apis {
                        if let Some(api_id) = &api.id {
                            // Get resources for this API
                            match client.get_resources().rest_api_id(api_id).send().await {
                                Ok(resources_response) => {
                                    if let Some(items) = resources_response.items {
                                        for resource in items {
                                            match self.create_resource_resource(&resource, &api, aws_account_dto, sync_id).await {
                                                Ok(resource_model) => all_resources.push(resource_model),
                                                Err(e) => error!("Failed to create resource resource: {}", e),
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to get resources for API {}: {}", api_id, e);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to get REST APIs for resources: {}", e);
            }
        }

        info!("Synced {} resources", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Sync API Gateway Methods
    pub async fn sync_methods(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing API Gateway methods for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_api_gateway_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // First get all REST APIs
        match client.get_rest_apis().send().await {
            Ok(response) => {
                if let Some(apis) = response.items {
                    for api in apis {
                        if let Some(api_id) = &api.id {
                            // Get resources for this API
                            match client.get_resources().rest_api_id(api_id).send().await {
                                Ok(resources_response) => {
                                    if let Some(resources) = resources_response.items {
                                        for resource in resources {
                                            if let Some(resource_id) = &resource.id {
                                                // Get methods for this resource
                                                match client.get_resource().rest_api_id(api_id).resource_id(resource_id).send().await {
                                                    Ok(resource_detail) => {
                                                        if let Some(methods) = &resource_detail.resource_methods {
                                                            for (method_name, method) in methods {
                                                                match self.create_method_resource(method_name, method, &resource, &api, aws_account_dto, sync_id).await {
                                                                    Ok(method_resource) => all_resources.push(method_resource),
                                                                    Err(e) => error!("Failed to create method resource: {}", e),
                                                                }
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to get resource detail for {}/{}: {}", api_id, resource_id, e);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to get resources for API {}: {}", api_id, e);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to get REST APIs for methods: {}", e);
            }
        }

        info!("Synced {} methods", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Create REST API resource from AWS SDK model
    async fn create_rest_api_resource(
        &self,
        api: &RestApi,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let resource_id = api.id.as_ref()
            .ok_or_else(|| AppError::Validation("API Gateway REST API ID missing".to_string()))?;

        let arn = format!(
            "arn:aws:apigateway:{}::/restapis/{}",
            aws_account_dto.default_region,
            resource_id
        );

        let name = api.name.as_ref()
            .unwrap_or(&"unknown".to_string())
            .clone();

        // Extract tags
        let tags = serde_json::json!({});

        // Build resource data
        let resource_data = serde_json::json!({
            "description": api.description,
            "created_date": api.created_date.map(|d| d.to_chrono_utc()),
            "api_key_source_type": api.api_key_source.as_ref().map(|s| s.as_str()),
            "endpoint_configuration": api.endpoint_configuration.as_ref().map(|config| {
                serde_json::json!({
                    "types": config.types.as_ref().map(|types| types.iter().map(|t| t.as_str()).collect::<Vec<_>>()),
                    "vpc_endpoint_ids": config.vpc_endpoint_ids
                })
            }),
            "disable_execute_api_endpoint": api.disable_execute_api_endpoint,
            "binary_media_types": api.binary_media_types,
            "minimum_compression_size": api.minimum_compression_size,
            "policy": api.policy,
            "tags": api.tags,
            "warnings": api.warnings,
            "version": api.version
        });

        let resource_dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::ApiGatewayRestApi.to_string(),
            resource_id: resource_id.to_string(),
            arn: arn.clone(),
            name: Some(name),
            tags,
            resource_data,
        };

        Ok(resource_dto)
    }

    /// Create Stage resource from AWS SDK model
    async fn create_stage_resource(
        &self,
        stage: &Stage,
        api: &RestApi,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let api_id = api.id.as_ref()
            .ok_or_else(|| AppError::Validation("API ID missing for stage".to_string()))?;

        let stage_name = stage.stage_name.as_ref()
            .ok_or_else(|| AppError::Validation("Stage name missing".to_string()))?;

        let resource_id = format!("{}/{}", api_id, stage_name);

        let arn = format!(
            "arn:aws:apigateway:{}::/restapis/{}/stages/{}",
            aws_account_dto.default_region,
            api_id,
            stage_name
        );

        let name = stage_name.clone();

        // Extract tags
        let tags = serde_json::json!({});

        // Build resource data
        let resource_data = serde_json::json!({
            "rest_api_id": api_id,
            "deployment_id": stage.deployment_id,
            "client_certificate_id": stage.client_certificate_id,
            "cache_cluster_enabled": stage.cache_cluster_enabled,
            "cache_cluster_size": stage.cache_cluster_size.as_ref().map(|s| s.as_str()),
            "cache_cluster_status": stage.cache_cluster_status.as_ref().map(|s| s.as_str()),
            "variables": stage.variables,
            "documentation_version": stage.documentation_version,
            "access_log_settings": stage.access_log_settings.as_ref().map(|settings| {
                serde_json::json!({
                    "format": settings.format,
                    "destination_arn": settings.destination_arn
                })
            }),
            "canary_settings": stage.canary_settings.as_ref().map(|settings| {
                serde_json::json!({
                    "percent_traffic": settings.percent_traffic,
                    "deployment_id": settings.deployment_id,
                    "stage_variable_overrides": settings.stage_variable_overrides,
                    "use_stage_cache": settings.use_stage_cache
                })
            }),
            "tracing_enabled": stage.tracing_enabled,
            "web_acl_arn": stage.web_acl_arn,
            "tags": stage.tags,
            "created_date": stage.created_date.map(|d| d.to_chrono_utc()),
            "last_updated_date": stage.last_updated_date.map(|d| d.to_chrono_utc())
        });

        let resource_dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::ApiGatewayStage.to_string(),
            resource_id: resource_id.to_string(),
            arn: arn.clone(),
            name: Some(name),
            tags,
            resource_data,
        };

        Ok(resource_dto)
    }

    /// Create Resource resource from AWS SDK model
    async fn create_resource_resource(
        &self,
        resource: &Resource,
        api: &RestApi,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let api_id = api.id.as_ref()
            .ok_or_else(|| AppError::Validation("API ID missing for resource".to_string()))?;

        let resource_id = resource.id.as_ref()
            .ok_or_else(|| AppError::Validation("Resource ID missing".to_string()))?;

        let full_resource_id = format!("{}/{}", api_id, resource_id);

        let arn = format!(
            "arn:aws:apigateway:{}::/restapis/{}/resources/{}",
            aws_account_dto.default_region,
            api_id,
            resource_id
        );

        let name = resource.path_part.as_ref()
            .unwrap_or(&"unknown".to_string())
            .clone();

        // Extract tags
        let tags = serde_json::json!({});

        // Build resource data
        let resource_data = serde_json::json!({
            "rest_api_id": api_id,
            "parent_id": resource.parent_id,
            "path_part": resource.path_part,
            "path": resource.path,
            "resource_methods": resource.resource_methods.as_ref().map(|methods| {
                methods.keys().collect::<Vec<_>>()
            })
        });

        let resource_dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::ApiGatewayResource.to_string(),
            resource_id: full_resource_id.to_string(),
            arn: arn.clone(),
            name: Some(name),
            tags,
            resource_data,
        };

        Ok(resource_dto)
    }

    /// Create Method resource from AWS SDK model
    async fn create_method_resource(
        &self,
        method_name: &str,
        method: &Method,
        resource: &Resource,
        api: &RestApi,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let api_id = api.id.as_ref()
            .ok_or_else(|| AppError::Validation("API ID missing for method".to_string()))?;

        let resource_id = resource.id.as_ref()
            .ok_or_else(|| AppError::Validation("Resource ID missing for method".to_string()))?;

        let full_method_id = format!("{}/{}/{}/{}", api_id, resource_id, method_name, method_name);

        let arn = format!(
            "arn:aws:apigateway:{}::/restapis/{}/resources/{}/methods/{}",
            aws_account_dto.default_region,
            api_id,
            resource_id,
            method_name
        );

        let name = format!("{}/{}", resource.path.as_ref().unwrap_or(&"unknown".to_string()), method_name);

        // Extract tags
        let tags = serde_json::json!({});

        // Build resource data
        let resource_data = serde_json::json!({
            "rest_api_id": api_id,
            "resource_id": resource_id,
            "http_method": method_name,
            "authorization_type": method.authorization_type,
            "authorizer_id": method.authorizer_id,
            "api_key_required": method.api_key_required,
            "request_validator_id": method.request_validator_id,
            "operation_name": method.operation_name,
            "request_parameters": method.request_parameters,
            "request_models": method.request_models,
            "method_responses": method.method_responses.as_ref().map(|responses| {
                responses.keys().collect::<Vec<_>>()
            })
        });

        let resource_dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::ApiGatewayMethod.to_string(),
            resource_id: full_method_id.to_string(),
            arn: arn.clone(),
            name: Some(name),
            tags,
            resource_data,
        };

        Ok(resource_dto)
    }
}