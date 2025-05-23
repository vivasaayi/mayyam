use std::sync::Arc;
use serde_json::Value;
use crate::errors::AppError;
use crate::services::AwsService;
use crate::services::aws::client_factory::AwsClientFactory;

/// Base service for AWS Cost Explorer functionality
pub struct AwsCostService {
    aws_service: Arc<AwsService>,
}

impl AwsCostService {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Create a Cost Explorer client with the given profile and region
    pub(crate) async fn create_client(&self, profile: Option<&str>, region: &str) 
        -> Result<aws_sdk_costexplorer::Client, AppError> {
        self.aws_service.create_cost_explorer_client(profile, region).await
    }
}
