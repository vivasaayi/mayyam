use std::sync::Arc;
use aws_sdk_rds::Client as RdsClient;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

// Control plane implementation for RDS
pub struct RdsControlPlane {
    aws_service: Arc<AwsService>,
}

impl RdsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_instances(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_instances_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_instances_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_rds_client_with_auth(profile, region, account_auth).await?;
        self.sync_instances_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_instances_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: RdsClient) -> Result<Vec<AwsResourceModel>, AppError> {
        let mut instances = Vec::new();
        let instance = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: "RdsInstance".to_string(),
            resource_id: "sample-postgres-db".to_string(),
            arn: format!("arn:aws:rds:{}:{}:db:sample-postgres-db", region, account_id),
            name: Some("Sample PostgreSQL Database".to_string()),
            tags: json!({"Name": "Sample PostgreSQL Database", "Environment": "Development"}),
            resource_data: json!({
                "db_instance_identifier": "sample-postgres-db",
                "engine": "postgres",
                "engine_version": "14.7",
                "instance_class": "db.t3.micro",
                "allocated_storage": 20,
                "endpoint": {
                    "address": format!("sample-postgres-db.{}.{}.rds.amazonaws.com", region, account_id),
                    "port": 5432,
                    "hosted_zone_id": "ABCDEFGH12345"
                },
                "status": "available",
                "availability_zone": format!("{}a", region),
                "multi_az": false,
                "storage_type": "gp2",
                "backup_retention_period": 7
            }),
        };
        instances.push(instance);

        Ok(instances.into_iter().map(|i| i.into()).collect())
    }
}