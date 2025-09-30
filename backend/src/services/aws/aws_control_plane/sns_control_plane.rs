use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;
use std::sync::Arc;

pub struct SnsControlPlane {
    aws_service: Arc<AwsService>,
}

impl SnsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_topics(
        &self,
        account_id: &str,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Vec<aws_resource::Model>, AppError> {
        let client = self.aws_service.create_sns_client(aws_account_dto).await?;

        let repo = &self.aws_service.aws_resource_repo;

        // Get SNS topics from AWS
        let mut topics = Vec::new();
        let mut next_token = None;

        // Paginate through all topics
        loop {
            // Build list topics request
            let mut request = client.list_topics();

            // Add next token if there's pagination
            if let Some(token) = &next_token {
                request = request.next_token(token);
            }

            // Send request to AWS
            let response = request.send().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to list SNS topics: {}", e))
            })?;

            // Process topics in the response
            for aws_topic in response.topics() {
                if let Some(topic_arn) = aws_topic.topic_arn() {
                    // Extract the topic name from ARN
                    let topic_name = topic_arn.split(':').last().unwrap_or_default();

                    // Get topic attributes
                    let attrs_response = client
                        .get_topic_attributes()
                        .topic_arn(topic_arn)
                        .send()
                        .await
                        .map_err(|e| {
                            AppError::ExternalService(format!(
                                "Failed to get attributes for topic {}: {}",
                                topic_arn, e
                            ))
                        })?;

                    // Copy attributes to a local HashMap for easier use
                    let mut attributes = std::collections::HashMap::new();
                    if let Some(attrs) = attrs_response.attributes() {
                        for (key, value) in attrs {
                            attributes.insert(key.to_string(), value.clone());
                        }
                    }

                    // Get topic tags
                    let tags_response = client
                        .list_tags_for_resource()
                        .resource_arn(topic_arn)
                        .send()
                        .await
                        .map_err(|e| {
                            AppError::ExternalService(format!(
                                "Failed to get tags for topic {}: {}",
                                topic_arn, e
                            ))
                        })?;

                    // Process tags
                    let mut tags_map = serde_json::Map::new();
                    let mut name = None;

                    // if let Some(tag_list) = tags_response.tags() {
                    //     for tag in tag_list {
                    //         if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    //             if key == "Name" {
                    //                 name = Some(value.to_string());
                    //             }
                    //             tags_map.insert(key.to_string(), json!(value));
                    //         }
                    //     }
                    // }

                    // If no name tag was found, use the topic name or display name
                    if name.is_none() {
                        if let Some(display_name) = attributes.get("DisplayName") {
                            name = Some(display_name.to_string());
                        } else {
                            name = Some(topic_name.to_string());
                        }
                    }

                    // Build topic data
                    let mut topic_data = serde_json::Map::new();

                    topic_data.insert("topic_arn".to_string(), json!(topic_arn));

                    if let Some(display_name) = attributes.get("DisplayName") {
                        topic_data.insert("display_name".to_string(), json!(display_name));
                    }

                    if let Some(subscriptions_confirmed) = attributes.get("SubscriptionsConfirmed")
                    {
                        if let Ok(count) = subscriptions_confirmed.parse::<i64>() {
                            topic_data.insert("subscriptions_confirmed".to_string(), json!(count));
                        }
                    }

                    if let Some(subscriptions_pending) = attributes.get("SubscriptionsPending") {
                        if let Ok(count) = subscriptions_pending.parse::<i64>() {
                            topic_data.insert("subscriptions_pending".to_string(), json!(count));
                        }
                    }

                    // Add delivery policy if available
                    if let Some(delivery_policy) = attributes.get("EffectiveDeliveryPolicy") {
                        if let Ok(policy_json) = serde_json::from_str(delivery_policy) {
                            topic_data.insert("effective_delivery_policy".to_string(), policy_json);
                        }
                    }

                    // Create resource DTO
                    let topic_dto = AwsResourceDto {
                        id: None,
                        account_id: aws_account_dto.account_id.clone(),
                        profile: aws_account_dto.profile.clone(),
                        region: aws_account_dto.default_region.clone(),
                        resource_type: AwsResourceType::SnsTopics.to_string(),
                        resource_id: topic_name.to_string(),
                        arn: topic_arn.to_string(),
                        name,
                        tags: serde_json::Value::Object(tags_map),
                        resource_data: serde_json::Value::Object(topic_data),
                        sync_id: None,
                    };

                    // Save to database
                    let saved_topic = match repo.find_by_arn(&topic_arn).await? {
                        Some(existing) => repo.update(existing.id, &topic_dto).await?,
                        None => repo.create(&topic_dto).await?,
                    };

                    topics.push(saved_topic);
                }
            }

            // Check if there are more topics to fetch
            next_token = response.next_token().map(|s| s.to_string());
            if next_token.is_none() {
                break;
            }
        }

        Ok(topics)
    }
}
