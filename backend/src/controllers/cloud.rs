use actix_web::{web, HttpResponse, Responder};
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use std::sync::Arc;
use uuid::Uuid;
use tracing::info;
use chrono::Utc;

use crate::services::aws::{
    AwsControlPlane, AwsCostService, AwsDataPlane,
};
use crate::models::aws_resource::{AwsResourceQuery, AwsResourceType};
use crate::services::aws::aws_data_plane::cloudwatch::{CloudWatchService, CloudWatchMetrics, CloudWatchLogs, CloudWatchLogsRequest, CloudWatchMetricsRequest};
use crate::services::aws::aws_data_plane::cost_explorer::CostAndUsage;
use crate::services::aws::aws_data_plane::sqs_data_plane::SqsDataPlane;
use crate::services::aws::aws_types::dynamodb::{DynamoDBGetItemRequest, DynamoDBPutItemRequest, DynamoDBQueryRequest};
use crate::services::aws::aws_types::sqs::{SqsReceiveMessageRequest, SqsSendMessageRequest};
use crate::services::aws::aws_data_plane::dynamodb_data_plane::DynamoDBDataPlane;
use crate::services::aws::aws_types::kinesis::{
    KinesisPutRecordRequest, KinesisCreateStreamRequest, KinesisDeleteStreamRequest, 
    KinesisDescribeStreamRequest, KinesisListStreamsRequest, KinesisUpdateShardCountRequest,
    KinesisRetentionPeriodRequest, KinesisEnhancedMonitoringRequest, KinesisListShardsRequest,
    KinesisPutRecordsRequest, KinesisGetRecordsRequest, KinesisGetShardIteratorRequest,
    KinesisPutRecordsResponse, KinesisGetRecordsResponse, KinesisGetShardIteratorResponse
};
use crate::services::aws::aws_data_plane::kinesis_data_plane::KinesisDataPlane;
use crate::services::aws::aws_control_plane::kinesis_control_plane::KinesisControlPlane;
use crate::services::aws::aws_types::s3::{S3GetObjectRequest, S3PutObjectRequest};
use crate::services::aws::aws_data_plane::s3_data_plane::S3DataPlane;
use crate::services::aws::aws_types::resource_sync::ResourceSyncRequest;

// AWS Control Plane operations
pub async fn sync_aws_resources(
    req: web::Json<ResourceSyncRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    info!("Syncing AWS resources for account {}", req.account_id);
    
    let response = aws_control_plane.sync_resources(&req).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

// AWS EC2 specific endpoints
pub async fn list_ec2_instances(
    path: web::Path<(String, String)>,
    query: web::Query<AwsResourceQuery>,
    aws_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (account_id, region) = path.into_inner();
    let mut query_params = query.into_inner();
    
    // Set the query params specific to EC2 instances
    query_params.account_id = Some(account_id);
    query_params.region = Some(region);
    query_params.resource_type = Some(AwsResourceType::EC2Instance.to_string());
    
    let resources = aws_repo.search(&query_params).await?;
    
    Ok(HttpResponse::Ok().json(resources))
}

// AWS S3 specific endpoints
pub async fn list_s3_buckets(
    path: web::Path<String>,
    query: web::Query<AwsResourceQuery>,
    aws_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let account_id = path.into_inner();
    let mut query_params = query.into_inner();
    
    // S3 buckets are global, so we don't filter by region
    query_params.account_id = Some(account_id);
    query_params.resource_type = Some(AwsResourceType::S3Bucket.to_string());
    
    let resources = aws_repo.search(&query_params).await?;
    
    Ok(HttpResponse::Ok().json(resources))
}

// AWS RDS specific endpoints
pub async fn list_rds_instances(
    path: web::Path<(String, String)>,
    query: web::Query<AwsResourceQuery>,
    aws_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (account_id, region) = path.into_inner();
    let mut query_params = query.into_inner();
    
    // Set the query params specific to RDS instances
    query_params.account_id = Some(account_id);
    query_params.region = Some(region);
    query_params.resource_type = Some(AwsResourceType::RdsInstance.to_string());
    
    let resources = aws_repo.search(&query_params).await?;
    
    Ok(HttpResponse::Ok().json(resources))
}

// AWS DynamoDB specific endpoints
pub async fn list_dynamodb_tables(
    path: web::Path<(String, String)>,
    query: web::Query<AwsResourceQuery>,
    aws_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (account_id, region) = path.into_inner();
    let mut query_params = query.into_inner();
    
    // Set the query params specific to DynamoDB tables
    query_params.account_id = Some(account_id);
    query_params.region = Some(region);
    query_params.resource_type = Some(AwsResourceType::DynamoDbTable.to_string());
    
    let resources = aws_repo.search(&query_params).await?;
    
    Ok(HttpResponse::Ok().json(resources))
}

// Generic AWS resource search endpoint
pub async fn search_aws_resources(
    query: web::Query<AwsResourceQuery>,
    aws_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query_params = query.into_inner();
    let resources = aws_repo.search(&query_params).await?;
    
    Ok(HttpResponse::Ok().json(resources))
}

// Get a specific AWS resource by ID
pub async fn get_aws_resource(
    path: web::Path<Uuid>,
    aws_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let resource_id = path.into_inner();
    let resource = aws_repo.find_by_id(resource_id).await?
        .ok_or_else(|| AppError::NotFound(format!("AWS resource with ID {} not found", resource_id)))?;
    
    Ok(HttpResponse::Ok().json(resource))
}

// AWS Cost functions
pub async fn get_aws_cost_and_usage(
    path: web::Path<(String, String)>,
    query: web::Query<serde_json::Value>,
    aws_cost_service: web::Data<Arc<AwsCostService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (account_id, region) = path.into_inner();
    let query_params = query.into_inner();
    
    // Extract date from query parameters
    let date = query_params.get("date")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation("date parameter is required".to_string()))?;
    
    let profile = query_params.get("profile")
        .and_then(|v| v.as_str());
    
    let group_by = None; // You can add group by options if needed
    let cost_data = aws_cost_service
        .get_cost_for_date(&account_id, profile, &region, date, group_by)
        .await?;
    
    Ok(HttpResponse::Ok().json(cost_data))
}

// AWS Data Plane operations

// S3 data plane operations
pub async fn s3_get_object(
    path: web::Path<(String, String, String)>,
    aws_data_plane: web::Data<Arc<S3DataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, bucket, key) = path.into_inner();
    
    // For S3, region doesn't matter as much since buckets are global
    let region = "us-east-1"; // This could be a parameter too
    
    let request = S3GetObjectRequest {
        bucket,
        key,
    };
    
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    let response = aws_data_plane.as_ref().get_object(profile_opt, region, &request).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn s3_put_object(
    path: web::Path<(String, String)>,
    req: web::Json<S3PutObjectRequest>,
    aws_data_plane: web::Data<Arc<S3DataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_data_plane.put_object(profile_opt, &region, &req).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

// DynamoDB data plane operations
pub async fn dynamodb_get_item(
    path: web::Path<(String, String, String)>,
    req: web::Json<DynamoDBGetItemRequest>,
    aws_data_plane: web::Data<Arc<DynamoDBDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region, table) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    // Override the table name in the path
    let mut request = req.into_inner();
    request.table_name = table;
    
    let response = aws_data_plane.get_item(profile_opt, &region, &request).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn dynamodb_put_item(
    path: web::Path<(String, String, String)>,
    req: web::Json<DynamoDBPutItemRequest>,
    aws_data_plane: web::Data<Arc<DynamoDBDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region, table) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    // Override the table name in the path
    let mut request = req.into_inner();
    request.table_name = table;
    
    let response = aws_data_plane.put_item(profile_opt, &region, &request).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn dynamodb_query(
    path: web::Path<(String, String, String)>,
    req: web::Json<DynamoDBQueryRequest>,
    aws_data_plane: web::Data<Arc<DynamoDBDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region, table) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    // Override the table name in the path
    let mut request = req.into_inner();
    request.table_name = table;
    
    let response = aws_data_plane.query(profile_opt, &region, &request).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

// SQS data plane operations
pub async fn sqs_send_message(
    path: web::Path<(String, String)>,
    req: web::Json<SqsSendMessageRequest>,
    aws_data_plane: web::Data<Arc<SqsDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_data_plane.send_message(profile_opt, &region, &req).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn sqs_receive_messages(
    path: web::Path<(String, String)>,
    req: web::Json<SqsReceiveMessageRequest>,
    aws_data_plane: web::Data<Arc<SqsDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_data_plane.receive_messages(profile_opt, &region, &req).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

// Kinesis data plane operations
pub async fn kinesis_put_record(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisPutRecordRequest>,
    aws_data_plane: web::Data<Arc<KinesisDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_data_plane.put_record(profile_opt, &region, &req).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_create_stream(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisCreateStreamRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };

    let response = aws_control_plane.kinesis_create_stream(profile_opt, &region, &req).await?;

    Ok(HttpResponse::Created().json(response))
}

pub async fn kinesis_delete_stream(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisDeleteStreamRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };

    let response = aws_control_plane.kinesis_delete_stream(profile_opt, &region, &req).await?;

    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_describe_stream(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisDescribeStreamRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };

    let response = aws_control_plane.kinesis_describe_stream(profile_opt, &region, &req).await?;

    Ok(HttpResponse::Ok().json(response))
}

// Additional Kinesis Control Plane Endpoints
pub async fn kinesis_list_streams(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisListStreamsRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_list_streams(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_describe_limits(
    path: web::Path<(String, String)>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_describe_limits(profile_opt, &region).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_describe_stream_summary(
    path: web::Path<(String, String, String)>, // (profile, region, stream_name)
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region, stream_name) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_describe_stream_summary(profile_opt, &region, &stream_name).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_update_shard_count(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisUpdateShardCountRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_update_shard_count(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_increase_retention_period(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisRetentionPeriodRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_increase_retention_period(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_decrease_retention_period(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisRetentionPeriodRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_decrease_retention_period(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_enable_enhanced_monitoring(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisEnhancedMonitoringRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_enable_enhanced_monitoring(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_disable_enhanced_monitoring(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisEnhancedMonitoringRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_disable_enhanced_monitoring(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_list_shards(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisListShardsRequest>,
    aws_control_plane: web::Data<Arc<AwsControlPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_control_plane.kinesis_list_shards(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

// Kinesis Data Plane Endpoints
pub async fn kinesis_put_records(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisPutRecordsRequest>,
    aws_data_plane: web::Data<Arc<AwsDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_data_plane.kinesis_put_records(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_get_records(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisGetRecordsRequest>,
    aws_data_plane: web::Data<Arc<AwsDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_data_plane.kinesis_get_records(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn kinesis_get_shard_iterator(
    path: web::Path<(String, String)>,
    req: web::Json<KinesisGetShardIteratorRequest>,
    aws_data_plane: web::Data<Arc<AwsDataPlane>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region) = path.into_inner();
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let response = aws_data_plane.kinesis_get_shard_iterator(profile_opt, &region, &req).await?;
    Ok(HttpResponse::Ok().json(response))
}

// Placeholder for cloud controller functionality
pub async fn list_providers(_claims: web::ReqData<Claims>) -> Result<impl Responder, AppError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Cloud providers API - Not yet implemented"
    })))
}

// CloudWatch metrics operations
pub async fn get_cloudwatch_metrics(
    path: web::Path<(String, String, String, String)>,
    req: web::Query<serde_json::Value>,
    cloudwatch_service: web::Data<Arc<CloudWatchService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region, resource_type, resource_id) = path.into_inner();
    let query_params = req.into_inner();
    
    // Get start and end times from query parameters
    let start_time = query_params.get("start_time")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    
    let end_time = query_params.get("end_time")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now());
    
    // Get period (in seconds)
    let period = query_params.get("period")
        .and_then(|v| v.as_i64())
        .unwrap_or(60) as i32;
    
    // Parse metrics
    let metrics = match query_params.get("metrics") {
        Some(m) if m.is_array() => {
            m.as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        },
        _ => {
            // Default metrics based on resource type
            match resource_type.as_str() {
                "EC2Instance" => vec![
                    "CPUUtilization".to_string(),
                    "NetworkIn".to_string(),
                    "NetworkOut".to_string(),
                    "DiskReadOps".to_string(),
                    "DiskWriteOps".to_string(),
                ],
                "ElasticacheCluster" => vec![
                    "CPUUtilization".to_string(),
                    "NetworkBytesIn".to_string(),
                    "NetworkBytesOut".to_string(),
                    "CacheHits".to_string(),
                    "CacheMisses".to_string(),
                ],
                _ => vec!["CPUUtilization".to_string()],
            }
        }
    };

    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let request = CloudWatchMetricsRequest {
        resource_id,
        resource_type,
        region: region.clone(),
        metrics,
        period,
        start_time,
        end_time,
    };

    // Access the inner CloudWatchService and use the trait method
    let result = cloudwatch_service.get_metrics(profile_opt, &region, &request).await?;
    
    Ok(HttpResponse::Ok().json(result))
}

// CloudWatch logs operations
pub async fn get_cloudwatch_logs(
    path: web::Path<(String, String, String)>,
    req: web::Query<serde_json::Value>,
    cloudwatch_service: web::Data<Arc<CloudWatchService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region, log_group) = path.into_inner();
    let query_params = req.into_inner();
    
    // Get start and end times from query parameters
    let start_time = query_params.get("start_time")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    
    let end_time = query_params.get("end_time")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now());
    
    // Filter pattern
    let filter_pattern = query_params.get("filter_pattern")
        .and_then(|v| v.as_str())
        .map(String::from);
    
    let profile_opt = if profile == "default" { None } else { Some(profile.as_str()) };
    
    let request = CloudWatchLogsRequest {
        log_group_name: log_group,
        start_time,
        end_time,
        filter_pattern,
        limit: Some(1000), // Add a default limit
    };
    
    // Access the inner CloudWatchService and use the trait method
    let result = cloudwatch_service.get_logs(profile_opt, &region, &request.log_group_name).await?;
    
    Ok(HttpResponse::Ok().json(result))
}

// Schedule metrics collection
pub async fn schedule_metrics_collection(
    path: web::Path<(String, String, String, String)>,
    req: web::Json<serde_json::Value>,
    _cloudwatch_service: web::Data<Arc<CloudWatchService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (profile, region, resource_type, resource_id) = path.into_inner();
    let body = req.into_inner();
    
    // Get interval in seconds
    let interval_seconds = body.get("interval_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(300); // Default to 5 minutes
    
    // This functionality is not yet implemented
    // Return a message indicating this
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Metrics collection scheduling is not yet implemented",
        "interval_seconds": interval_seconds,
        "resource_id": resource_id,
        "resource_type": resource_type,
        "profile": profile,
        "region": region
    })))
}

// List ElastiCache clusters
pub async fn list_elasticache_clusters(
    path: web::Path<(String, String)>,
    query: web::Query<AwsResourceQuery>,
    aws_repo: web::Data<Arc<crate::repositories::aws_resource::AwsResourceRepository>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (account_id, region) = path.into_inner();
    let mut query_params = query.into_inner();
    
    // Set the query params specific to ElastiCache clusters
    query_params.account_id = Some(account_id);
    query_params.region = Some(region);
    query_params.resource_type = Some(AwsResourceType::ElasticacheCluster.to_string());
    
    let resources = aws_repo.search(&query_params).await?;
    
    Ok(HttpResponse::Ok().json(resources))
}
