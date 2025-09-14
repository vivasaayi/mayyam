use chrono::{DateTime, Utc, Duration};
use serde_json::{json, Value};
use tracing::{debug, error};

use aws_sdk_cloudwatchlogs::{
    Client as CloudWatchLogsClient,
    types::OrderBy,
    operation::filter_log_events::{
        FilterLogEventsInput as FilterLogEventsRequest,
    },
};


use crate::{errors::AppError, models::aws_account::AwsAccountDto};
use super::base::CloudWatchService;
use super::types::CloudWatchLogsRequest;

pub trait CloudWatchLogs {
    async fn get_logs(
        &self,
        aws_account_dto: &AwsAccountDto,
        log_group: &str
    ) -> Result<Value, AppError>;
    
    async fn get_filtered_logs(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &CloudWatchLogsRequest
    ) -> Result<Value, AppError>;
}

impl CloudWatchLogs for CloudWatchService {
    async fn get_logs(&self, aws_account_dto: &AwsAccountDto, log_group: &str) -> Result<Value, AppError> {
        let client = self.create_cloudwatch_logs_client(aws_account_dto).await?;

        let start_time = (Utc::now() - Duration::hours(1)).timestamp_millis();
        let end_time = Utc::now().timestamp_millis();
        
        debug!("Fetching CloudWatch logs for group: {}", log_group);
        
        let response = client.filter_log_events()
            .log_group_name(log_group)
            .start_time(start_time as i64)
            .end_time(end_time as i64)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get CloudWatch logs: {}", e)))?;
            
        let mut events = Vec::new();
        for event in response.events() {
            let mut event_data = json!({});
            
            if let Some(timestamp) = event.timestamp() {
                let dt = DateTime::<Utc>::from_timestamp_millis(timestamp)
                    .unwrap_or_else(|| Utc::now());
                event_data["timestamp"] = json!(dt.to_rfc3339());
            }
            
            if let Some(message) = event.message() {
                // Try to parse message as JSON if possible
                if message.starts_with("{") && message.ends_with("}") {
                    if let Ok(json_value) = serde_json::from_str(message) {
                        event_data["message"] = json_value;
                    } else {
                        event_data["message"] = json!(message);
                    }
                } else {
                    event_data["message"] = json!(message);
                }
            }
            
            if let Some(ingestion_time) = event.ingestion_time() {
                let dt = DateTime::<Utc>::from_timestamp_millis(ingestion_time)
                    .unwrap_or_else(|| Utc::now());
                event_data["ingestionTime"] = json!(dt.to_rfc3339());
            }
            
            events.push(event_data);
        }
        
        Ok(json!({
            "events": events,
            "logGroupName": log_group,
            "startTime": start_time,
            "endTime": end_time
        }))
    }

    async fn get_filtered_logs(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &CloudWatchLogsRequest
    ) -> Result<Value, AppError> {
        let client = self.create_cloudwatch_logs_client(aws_account_dto).await?;
        
        let start_millis = request.start_time.timestamp_millis();
        let end_millis = request.end_time.timestamp_millis();
        
        let mut builder = client.filter_log_events()
            .log_group_name(&request.log_group_name)
            .start_time(start_millis as i64)
            .end_time(end_millis as i64);
            
        if let Some(filter) = &request.filter_pattern {
            builder = builder.filter_pattern(filter);
        }
        
        if let Some(limit) = request.limit {
            builder = builder.limit(limit);
        }
        
        let mut events = Vec::new();
        let mut token: Option<String> = None;
        
        loop {
            let mut current_req = builder.clone();
            if let Some(next_token) = &token {
                current_req = current_req.next_token(next_token);
            }
            
            let response = current_req
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Failed to get CloudWatch logs: {}", e)))?;

            for event in response.events() {
                let mut event_data = json!({});
                
                if let Some(timestamp) = event.timestamp() {
                    let dt = DateTime::<Utc>::from_timestamp_millis(timestamp)
                        .unwrap_or_else(|| Utc::now());
                    event_data["timestamp"] = json!(dt.to_rfc3339());
                }
                
                if let Some(message) = event.message() {
                    if message.starts_with("{") && message.ends_with("}") {
                        if let Ok(json_value) = serde_json::from_str(message) {
                            event_data["message"] = json_value;
                        } else {
                            event_data["message"] = json!(message);
                        }
                    } else {
                        event_data["message"] = json!(message);
                    }
                }
                
                if let Some(ingestion_time) = event.ingestion_time() {
                    let dt = DateTime::<Utc>::from_timestamp_millis(ingestion_time)
                        .unwrap_or_else(|| Utc::now());
                    event_data["ingestionTime"] = json!(dt.to_rfc3339());
                }
                
                events.push(event_data);
            }
            
            match response.next_token() {
                Some(next_token) => token = Some(next_token.to_string()),
                None => break,
            }
        }
        
        Ok(json!({
            "events": events,
            "logGroupName": request.log_group_name,
            "filterPattern": request.filter_pattern,
            "startTime": request.start_time,
            "endTime": request.end_time
        }))
    }
}
