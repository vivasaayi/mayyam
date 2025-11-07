use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::services::aws::aws_types::cloud_watch::{
    CloudWatchMetricsRequest, CloudWatchMetricsResult,
};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;
use std::sync::Arc;
use uuid;

// Data plane implementation for EC2
pub struct Ec2DataPlane {
    aws_service: Arc<AwsService>,
}

impl Ec2DataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn get_instance_metrics(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &CloudWatchMetricsRequest,
    ) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self
            .aws_service
            .create_cloudwatch_client(aws_account_dto)
            .await?;

        // Mock implementation for EC2 metrics
        Ok(CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics: vec![],
        })
    }

    pub async fn get_instance_status(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

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

    pub async fn get_instance_console_output(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_id: &str,
    ) -> Result<String, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        // Mock implementation
        Ok("Console output would appear here...".to_string())
    }

    pub async fn monitor_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_ids: &[String],
    ) -> Result<Vec<(String, bool)>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        // Mock implementation returning instance ID and monitoring state
        Ok(instance_ids.iter().map(|id| (id.clone(), true)).collect())
    }

    pub async fn unmonitor_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_ids: &[String],
    ) -> Result<Vec<(String, bool)>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        // Mock implementation returning instance ID and monitoring state
        Ok(instance_ids.iter().map(|id| (id.clone(), false)).collect())
    }
}
