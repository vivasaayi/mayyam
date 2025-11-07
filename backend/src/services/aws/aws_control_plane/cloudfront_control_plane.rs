use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel, AwsResourceType};
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::aws::service::AwsService;
use aws_sdk_cloudfront::types::DistributionSummary;
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct CloudFrontControlPlane {
    aws_service: Arc<AwsService>,
}

impl CloudFrontControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Sync CloudFront Distributions
    pub async fn sync_distributions(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing CloudFront distributions for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_cloudfront_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // Get all distributions
        let mut marker = None;
        loop {
            let mut request = client.list_distributions();
            if let Some(m) = &marker {
                request = request.marker(m);
            }

            match request.send().await {
                Ok(response) => {
                    if let Some(distribution_list) = &response.distribution_list {
                        if let Some(items) = &distribution_list.items {
                            for dist in items {
                                match self.create_distribution_resource(dist, aws_account_dto, sync_id).await {
                                    Ok(resource) => all_resources.push(resource),
                                    Err(e) => error!("Failed to create CloudFront distribution resource: {}", e),
                                }
                            }
                        }

                        // Check if there are more pages
                        if let Some(is_truncated) = distribution_list.is_truncated {
                            if is_truncated {
                                marker = distribution_list.next_marker.clone();
                                if marker.is_none() {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to list CloudFront distributions: {}", e);
                    break;
                }
            }
        }

        info!("Synced {} CloudFront distributions", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Create CloudFront distribution resource from AWS SDK model
    async fn create_distribution_resource(
        &self,
        dist: &DistributionSummary,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let resource_id = dist.id.as_ref()
            .ok_or_else(|| AppError::Validation("CloudFront distribution ID missing".to_string()))?;

        let arn = format!(
            "arn:aws:cloudfront::{}:distribution/{}",
            aws_account_dto.account_id,
            resource_id
        );

        // Get distribution comment as name, or use ID if not available
        let name = dist.comment.as_ref()
            .unwrap_or(&resource_id.clone())
            .clone();

        // Extract tags (CloudFront tags are retrieved separately)
        let tags = serde_json::json!({});

        // Build resource data
        let resource_data = serde_json::json!({
            "domain_name": dist.domain_name,
            "comment": dist.comment,
            "enabled": dist.enabled,
            "status": dist.status,
            "last_modified_time": dist.last_modified_time.map(|t| t.to_chrono_utc()),
            "origins": dist.origins.as_ref().map(|origins| {
                origins.items.as_ref().map(|items| {
                    items.iter().map(|origin| {
                        serde_json::json!({
                            "id": origin.id,
                            "domain_name": origin.domain_name,
                            "origin_path": origin.origin_path,
                            "connection_attempts": origin.connection_attempts,
                            "connection_timeout": origin.connection_timeout,
                            "custom_headers": origin.custom_headers.as_ref().map(|headers| {
                                headers.items.as_ref().map(|items| {
                                    items.iter().map(|header| {
                                        serde_json::json!({
                                            "name": header.name,
                                            "value": header.value
                                        })
                                    }).collect::<Vec<_>>()
                                })
                            })
                        })
                    }).collect::<Vec<_>>()
                })
            }),
            "default_cache_behavior": dist.default_cache_behavior.as_ref().map(|behavior| {
                serde_json::json!({
                    "target_origin_id": behavior.target_origin_id,
                    "viewer_protocol_policy": behavior.viewer_protocol_policy,
                    "trusted_signers": behavior.trusted_signers.as_ref().map(|signers| {
                        serde_json::json!({
                            "enabled": signers.enabled,
                            "quantity": signers.quantity,
                            "items": signers.items
                        })
                    }),
                    "forwarded_values": behavior.forwarded_values.as_ref().map(|values| {
                        serde_json::json!({
                            "query_string": values.query_string,
                            "cookies": values.cookies.as_ref().map(|cookies| {
                                serde_json::json!({
                                    "forward": cookies.forward,
                                    "whitelisted_names": cookies.whitelisted_names.as_ref().map(|names| {
                                        serde_json::json!({
                                            "quantity": names.quantity,
                                            "items": names.items
                                        })
                                    })
                                })
                            }),
                            "headers": values.headers.as_ref().map(|headers| {
                                serde_json::json!({
                                    "quantity": headers.quantity,
                                    "items": headers.items
                                })
                            }),
                            "query_string_cache_keys": values.query_string_cache_keys.as_ref().map(|keys| {
                                serde_json::json!({
                                    "quantity": keys.quantity,
                                    "items": keys.items
                                })
                            })
                        })
                    }),
                    "min_ttl": behavior.min_ttl,
                    "default_ttl": behavior.default_ttl,
                    "max_ttl": behavior.max_ttl
                })
            }),
            "cache_behaviors": dist.cache_behaviors.as_ref().map(|behaviors| {
                behaviors.items.as_ref().map(|items| {
                    items.iter().map(|behavior| {
                        serde_json::json!({
                            "path_pattern": behavior.path_pattern,
                            "target_origin_id": behavior.target_origin_id,
                            "viewer_protocol_policy": behavior.viewer_protocol_policy
                        })
                    }).collect::<Vec<_>>()
                })
            }),
            "custom_error_responses": dist.custom_error_responses.as_ref().map(|responses| {
                responses.items.as_ref().map(|items| {
                    items.iter().map(|response| {
                        serde_json::json!({
                            "error_code": response.error_code,
                            "response_page_path": response.response_page_path,
                            "response_code": response.response_code,
                            "error_caching_min_ttl": response.error_caching_min_ttl
                        })
                    }).collect::<Vec<_>>()
                })
            }),
            "aliases": dist.aliases.as_ref().map(|aliases| {
                serde_json::json!({
                    "quantity": aliases.quantity,
                    "items": aliases.items
                })
            }),
            "web_acl_id": dist.web_acl_id,
            "http_version": dist.http_version,
            "is_ipv6_enabled": dist.is_ipv6_enabled,
            "price_class": dist.price_class
        });

        let resource_dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: "global".to_string(), // CloudFront is global
            resource_type: AwsResourceType::CloudFrontDistribution.to_string(),
            resource_id: resource_id.to_string(),
            arn: arn.clone(),
            name: Some(name),
            tags,
            resource_data,
        };

        Ok(resource_dto)
    }
}