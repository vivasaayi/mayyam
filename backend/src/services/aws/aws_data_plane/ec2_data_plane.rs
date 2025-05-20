use std::sync::Arc;
use serde_json::json;
use crate::errors::AppError;
use crate::services::aws::aws_types::cloud_watch::{CloudWatchMetricsRequest, CloudWatchMetricsResult};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

// Data plane implementation for EC2
pub struct Ec2DataPlane {
    aws_service: Arc<AwsService>,
}

impl Ec2DataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn get_instance_metrics(&self, request: &CloudWatchMetricsRequest) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self.aws_service.create_cloudwatch_client(None, &request.region).await?;
        
        // Mock implementation for EC2 metrics
        Ok(CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics: vec![],
        })
    }

    pub async fn get_instance_status(&self, profile: Option<&str>, region: &str, instance_id: &str) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation
        let response = json!({
            "instance_id": instance_id,
            "instance_state": {
                "code": 16,
                "name": "running"
            },
            "system_status": {
                "status": "ok",
                "details": [{
                    "name": "reachability",
                    "status": "passed"
                }]
            },
            "instance_status": {
                "status": "ok",
                "details": [{
                    "name": "reachability",
                    "status": "passed"
                }]
            }
        });
        
        Ok(response)
    }

    pub async fn get_instance_console_output(&self, profile: Option<&str>, region: &str, instance_id: &str) -> Result<String, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation
        Ok("Console output would appear here...".to_string())
    }

    pub async fn monitor_instances(&self, profile: Option<&str>, region: &str, instance_ids: &[String]) -> Result<Vec<(String, bool)>, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation returning instance ID and monitoring state
        Ok(instance_ids.iter().map(|id| (id.clone(), true)).collect())
    }

    pub async fn unmonitor_instances(&self, profile: Option<&str>, region: &str, instance_ids: &[String]) -> Result<Vec<(String, bool)>, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation returning instance ID and monitoring state
        Ok(instance_ids.iter().map(|id| (id.clone(), false)).collect())
    }
}