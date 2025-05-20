use std::sync::Arc;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

pub struct OpenSearchControlPlane {
    aws_service: Arc<AwsService>,
}

impl OpenSearchControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_domains(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_domains_with_auth(account_id, profile, region, None).await
    }

    pub async fn sync_domains_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_opensearch_client_with_auth(profile, region, account_auth).await?;
        self.sync_domains_with_client(account_id, profile, region, client).await
    }

    pub async fn sync_domains_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, _client: aws_sdk_opensearch::Client) -> Result<Vec<AwsResourceModel>, AppError> {
        let repo = &self.aws_service.aws_resource_repo;
        
        let mut domains = Vec::new();
        
        let domain_data = json!({
            "domain_name": "sample-domain",
            "domain_id": format!("{}:sample-domain", account_id),
            "engine_version": "OpenSearch_2.5",
            "cluster_config": {
                "instance_type": "t3.small.search",
                "instance_count": 1,
                "dedicated_master_enabled": false,
                "zone_awareness_enabled": false
            },
            "ebs_options": {
                "ebs_enabled": true,
                "volume_type": "gp3",
                "volume_size": 10,
                "iops": 3000
            },
            "access_policies": "{\"Version\": \"2012-10-17\",\"Statement\":[]}",
            "snapshot_options": {
                "automated_snapshot_start_hour": 0
            },
            "vpc_options": null,
            "encryption_at_rest_options": {
                "enabled": true
            },
            "node_to_node_encryption_options": {
                "enabled": true
            },
            "advanced_options": {
                "rest.action.multi.allow_explicit_index": "true"
            },
            "endpoints": {
                "vpc": format!("vpc-sample-domain-{}.{}.es.amazonaws.com", account_id, region)
            }
        });
        
        let domain = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::OpenSearchDomain.to_string(),
            resource_id: "sample-domain".to_string(),
            arn: format!("arn:aws:es:{}:{}:domain/sample-domain", region, account_id),
            name: Some("Sample OpenSearch Domain".to_string()),
            tags: json!({"Name": "Sample OpenSearch Domain", "Environment": "Development"}),
            resource_data: domain_data,
        };
        
        let saved_domain = match repo.find_by_arn(&domain.arn).await? {
            Some(existing) => repo.update(existing.id, &domain).await?,
            None => repo.create(&domain).await?,
        };
        domains.push(saved_domain);
        
        Ok(domains)
    }
}