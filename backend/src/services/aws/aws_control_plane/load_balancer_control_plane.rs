use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel, AwsResourceType};
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::aws::service::AwsService;
use aws_sdk_elasticloadbalancingv2::types::LoadBalancer as ClassicLoadBalancer;
use aws_sdk_elasticloadbalancingv2::types::LoadBalancer as AlbLoadBalancer;
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct LoadBalancerControlPlane {
    aws_service: Arc<AwsService>,
}

impl LoadBalancerControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Sync Application Load Balancers (ALB)
    pub async fn sync_albs(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing ALBs for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_elbv2_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // Get all load balancers
        let mut next_marker = None;
        loop {
            let mut request = client.describe_load_balancers();
            if let Some(marker) = &next_marker {
                request = request.marker(marker);
            }

            match request.send().await {
                Ok(response) => {
                    if let Some(load_balancers) = response.load_balancers {
                        for lb in load_balancers {
                            if let Some(lb_type) = &lb.type_ {
                                // Only process Application Load Balancers
                                if lb_type.as_str() == "application" {
                                    match self.create_alb_resource(&lb, aws_account_dto, sync_id).await {
                                        Ok(resource) => all_resources.push(resource),
                                        Err(e) => error!("Failed to create ALB resource: {}", e),
                                    }
                                }
                            }
                        }
                    }

                    next_marker = response.next_marker;
                    if next_marker.is_none() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to describe ALBs: {}", e);
                    break;
                }
            }
        }

        info!("Synced {} ALBs", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Sync Network Load Balancers (NLB)
    pub async fn sync_nlbs(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing NLBs for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_elbv2_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // Get all load balancers
        let mut next_marker = None;
        loop {
            let mut request = client.describe_load_balancers();
            if let Some(marker) = &next_marker {
                request = request.marker(marker);
            }

            match request.send().await {
                Ok(response) => {
                    if let Some(load_balancers) = response.load_balancers {
                        for lb in load_balancers {
                            if let Some(lb_type) = &lb.type_ {
                                // Only process Network Load Balancers
                                if lb_type.as_str() == "network" {
                                    match self.create_nlb_resource(&lb, aws_account_dto, sync_id).await {
                                        Ok(resource) => all_resources.push(resource),
                                        Err(e) => error!("Failed to create NLB resource: {}", e),
                                    }
                                }
                            }
                        }
                    }

                    next_marker = response.next_marker;
                    if next_marker.is_none() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to describe NLBs: {}", e);
                    break;
                }
            }
        }

        info!("Synced {} NLBs", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Sync Classic Load Balancers (ELB)
    pub async fn sync_elbs(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing ELBs for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_elb_client(aws_account_dto).await?;
        let mut all_resources = Vec::new();

        // Get all classic load balancers
        match client.describe_load_balancers().send().await {
            Ok(response) => {
                if let Some(load_balancers) = response.load_balancer_descriptions {
                    for lb in load_balancers {
                        match self.create_elb_resource(&lb, aws_account_dto, sync_id).await {
                            Ok(resource) => all_resources.push(resource),
                            Err(e) => error!("Failed to create ELB resource: {}", e),
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to describe ELBs: {}", e);
            }
        }

        info!("Synced {} ELBs", all_resources.len());
        Ok(all_resources.into_iter().map(|r| r.into()).collect())
    }

    /// Create ALB resource from AWS SDK model
    async fn create_alb_resource(
        &self,
        lb: &AlbLoadBalancer,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let resource_id = lb.load_balancer_arn.as_ref()
            .ok_or_else(|| AppError::Validation("ALB ARN missing".to_string()))?
            .split(':')
            .last()
            .unwrap_or("unknown")
            .split('/')
            .last()
            .unwrap_or("unknown");

        let name = lb.load_balancer_name.as_ref()
            .unwrap_or(&"unknown".to_string())
            .clone();

        let arn = lb.load_balancer_arn.as_ref()
            .ok_or_else(|| AppError::Validation("ALB ARN missing".to_string()))?
            .clone();

        // Extract tags
        let tags = if let Some(tags_list) = &lb.tags {
            serde_json::json!(tags_list.iter()
                .filter_map(|tag| {
                    if let (Some(key), Some(value)) = (&tag.key, &tag.value) {
                        Some((key.clone(), value.clone()))
                    } else {
                        None
                    }
                })
                .collect::<std::collections::HashMap<_, _>>())
        } else {
            serde_json::json!({})
        };

        // Build resource data
        let resource_data = serde_json::json!({
            "dns_name": lb.dns_name,
            "canonical_hosted_zone_id": lb.canonical_hosted_zone_id,
            "vpc_id": lb.vpc_id,
            "state": lb.state.as_ref().map(|s| s.code.as_ref()),
            "scheme": lb.scheme,
            "load_balancer_type": lb.type_,
            "availability_zones": lb.availability_zones.as_ref().map(|azs|
                azs.iter().filter_map(|az| az.zone_name.clone()).collect::<Vec<_>>()
            ),
            "security_groups": lb.security_groups,
            "ip_address_type": lb.ip_address_type,
            "customer_owned_ipv4_pool": lb.customer_owned_ipv4_pool
        });

        let resource_dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::Alb.to_string(),
            resource_id: resource_id.to_string(),
            arn: arn.clone(),
            name: Some(name),
            tags,
            resource_data,
        };

        Ok(resource_dto)
    }

    /// Create NLB resource from AWS SDK model
    async fn create_nlb_resource(
        &self,
        lb: &AlbLoadBalancer,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let resource_id = lb.load_balancer_arn.as_ref()
            .ok_or_else(|| AppError::Validation("NLB ARN missing".to_string()))?
            .split(':')
            .last()
            .unwrap_or("unknown")
            .split('/')
            .last()
            .unwrap_or("unknown");

        let name = lb.load_balancer_name.as_ref()
            .unwrap_or(&"unknown".to_string())
            .clone();

        let arn = lb.load_balancer_arn.as_ref()
            .ok_or_else(|| AppError::Validation("NLB ARN missing".to_string()))?
            .clone();

        // Extract tags
        let tags = if let Some(tags_list) = &lb.tags {
            serde_json::json!(tags_list.iter()
                .filter_map(|tag| {
                    if let (Some(key), Some(value)) = (&tag.key, &tag.value) {
                        Some((key.clone(), value.clone()))
                    } else {
                        None
                    }
                })
                .collect::<std::collections::HashMap<_, _>>())
        } else {
            serde_json::json!({})
        };

        // Build resource data
        let resource_data = serde_json::json!({
            "dns_name": lb.dns_name,
            "canonical_hosted_zone_id": lb.canonical_hosted_zone_id,
            "vpc_id": lb.vpc_id,
            "state": lb.state.as_ref().map(|s| s.code.as_ref()),
            "scheme": lb.scheme,
            "load_balancer_type": lb.type_,
            "availability_zones": lb.availability_zones.as_ref().map(|azs|
                azs.iter().filter_map(|az| az.zone_name.clone()).collect::<Vec<_>>()
            ),
            "security_groups": lb.security_groups,
            "ip_address_type": lb.ip_address_type,
            "customer_owned_ipv4_pool": lb.customer_owned_ipv4_pool
        });

        let resource_dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::Nlb.to_string(),
            resource_id: resource_id.to_string(),
            arn: arn.clone(),
            name: Some(name),
            tags,
            resource_data,
        };

        Ok(resource_dto)
    }

    /// Create ELB resource from AWS SDK model
    async fn create_elb_resource(
        &self,
        lb: &ClassicLoadBalancer,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<AwsResourceDto, AppError> {
        let resource_id = lb.load_balancer_name.as_ref()
            .ok_or_else(|| AppError::Validation("ELB name missing".to_string()))?;

        let name = resource_id.clone();

        // Construct ARN for ELB (classic load balancers don't have ARNs in the same way)
        let arn = format!(
            "arn:aws:elasticloadbalancing:{}:{}:loadbalancer/{}",
            aws_account_dto.default_region,
            aws_account_dto.account_id,
            resource_id
        );

        // Extract tags (ELB tags are retrieved separately)
        let tags = serde_json::json!({});

        // Build resource data
        let resource_data = serde_json::json!({
            "dns_name": lb.dns_name,
            "canonical_hosted_zone_name": lb.canonical_hosted_zone_name,
            "canonical_hosted_zone_name_id": lb.canonical_hosted_zone_name_id,
            "vpc_id": lb.vpc_id,
            "load_balancer_type": "classic",
            "availability_zones": lb.availability_zones,
            "security_groups": lb.security_groups,
            "scheme": lb.scheme,
            "health_check": lb.health_check.as_ref().map(|hc| {
                serde_json::json!({
                    "target": hc.target,
                    "interval": hc.interval,
                    "timeout": hc.timeout,
                    "unhealthy_threshold": hc.unhealthy_threshold,
                    "healthy_threshold": hc.healthy_threshold
                })
            }),
            "created_time": lb.created_time.map(|t| t.to_chrono_utc())
        });

        let resource_dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: aws_account_dto.default_region.clone(),
            resource_type: AwsResourceType::Elb.to_string(),
            resource_id: resource_id.to_string(),
            arn: arn.clone(),
            name: Some(name),
            tags,
            resource_data,
        };

        Ok(resource_dto)
    }
}