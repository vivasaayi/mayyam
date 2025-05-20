use std::sync::Arc;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

pub struct SnsControlPlane {
    aws_service: Arc<AwsService>,
}

impl SnsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_topics(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<aws_resource::Model>, AppError> {
        self.sync_topics_with_auth(account_id, profile, region, None).await
    }

    pub async fn sync_topics_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<aws_resource::Model>, AppError> {
        let client = self.aws_service.create_sns_client_with_auth(profile, region, account_auth).await?;
        self.sync_topics_with_client(account_id, profile, region, client).await
    }

    pub async fn sync_topics_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, _client: aws_sdk_sns::Client) -> Result<Vec<aws_resource::Model>, AppError> {
        let repo = &self.aws_service.aws_resource_repo;
        
        // Sample topic data for now
        let mut topics = Vec::new();
        
        let standard_topic_data = json!({
            "topic_arn": format!("arn:aws:sns:{}:{}:sample-standard-topic", region, account_id),
            "display_name": "Sample Standard Topic",
            "subscriptions_confirmed": 2,
            "subscriptions_pending": 0,
            "effective_delivery_policy": {
                "defaultHealthyRetryPolicy": {
                    "minDelayTarget": 20,
                    "maxDelayTarget": 20,
                    "numRetries": 3,
                    "numMaxDelayRetries": 0,
                    "numNoDelayRetries": 0,
                    "numMinDelayRetries": 0,
                    "backoffFunction": "linear"
                }
            }
        });
        
        let standard_topic = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::SnsTopics.to_string(),
            resource_id: "sample-standard-topic".to_string(),
            arn: format!("arn:aws:sns:{}:{}:sample-standard-topic", region, account_id),
            name: Some("Sample Standard Topic".to_string()),
            tags: json!({"Name": "Sample Standard Topic", "Environment": "Development"}),
            resource_data: standard_topic_data,
        };
        
        // Save to database
        let saved_standard_topic = match repo.find_by_arn(&standard_topic.arn).await? {
            Some(existing) => repo.update(existing.id, &standard_topic).await?,
            None => repo.create(&standard_topic).await?,
        };
        topics.push(saved_standard_topic);
        
        Ok(topics)
    }
}