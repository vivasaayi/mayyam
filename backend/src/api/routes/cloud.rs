use actix_web::{web, HttpResponse};
use crate::controllers::cloud;
use crate::api::routes::aws_account;

pub fn configure(cfg: &mut web::ServiceConfig) {
    // General cloud provider routes
    let cloud_scope = web::scope("/api/cloud")
        .route("/providers", web::get().to(cloud::list_providers));
    
    // AWS resource management (control plane)
    let aws_scope = web::scope("/api/aws")
        // Resource syncing
        .route("/sync", web::post().to(cloud::sync_aws_resources))
        
        // General resource search
        .route("/resources", web::get().to(cloud::search_aws_resources))
        .route("/resources/{id}", web::get().to(cloud::get_aws_resource))
        
        // Include AWS account management
        .service(aws_account::configure())
        
        // EC2 instances
        .route("/accounts/{account_id}/regions/{region}/ec2", 
               web::get().to(cloud::list_ec2_instances))
        
        // ElastiCache/Redis clusters
        .route("/accounts/{account_id}/regions/{region}/elasticache", 
               web::get().to(cloud::list_elasticache_clusters))
               
        // S3 buckets (global resource)
        .route("/accounts/{account_id}/s3", 
               web::get().to(cloud::list_s3_buckets))
        
        // RDS instances
        .route("/accounts/{account_id}/regions/{region}/rds", 
               web::get().to(cloud::list_rds_instances))
        
        // DynamoDB tables
        .route("/accounts/{account_id}/regions/{region}/dynamodb", 
               web::get().to(cloud::list_dynamodb_tables))
        
        // CloudWatch metrics
        .route("/profiles/{profile}/regions/{region}/metrics/{resource_type}/{resource_id}", 
               web::get().to(cloud::get_cloudwatch_metrics))
               
        // CloudWatch logs
        .route("/profiles/{profile}/regions/{region}/logs/{log_group}", 
               web::get().to(cloud::get_cloudwatch_logs))
               
        // Schedule metrics collection
        .route("/profiles/{profile}/regions/{region}/schedule/{resource_type}/{resource_id}", 
               web::post().to(cloud::schedule_metrics_collection))
               
        // AWS cost
        .route("/accounts/{account_id}/regions/{region}/cost", 
               web::get().to(cloud::get_aws_cost_and_usage));
    
    // AWS data plane operations
    let aws_data_scope = web::scope("/api/aws-data")
        // S3 operations
        .route("/profiles/{profile}/s3/{bucket}/{key}", 
               web::get().to(cloud::s3_get_object))
        .route("/profiles/{profile}/regions/{region}/s3", 
               web::post().to(cloud::s3_put_object))
        
        // DynamoDB operations
        .route("/profiles/{profile}/regions/{region}/dynamodb/{table}/item", 
               web::get().to(cloud::dynamodb_get_item))
        .route("/profiles/{profile}/regions/{region}/dynamodb/{table}/item", 
               web::post().to(cloud::dynamodb_put_item))
        .route("/profiles/{profile}/regions/{region}/dynamodb/{table}/query", 
               web::post().to(cloud::dynamodb_query))
        
        // SQS operations
        .route("/profiles/{profile}/regions/{region}/sqs/send", 
               web::post().to(cloud::sqs_send_message))
        .route("/profiles/{profile}/regions/{region}/sqs/receive", 
               web::post().to(cloud::sqs_receive_messages))
        
        // Kinesis operations
        .route("/profiles/{profile}/regions/{region}/kinesis", 
               web::post().to(cloud::kinesis_put_record))
        .route("/profiles/{profile}/regions/{region}/kinesis/streams", 
               web::post().to(cloud::kinesis_create_stream))
        .route("/profiles/{profile}/regions/{region}/kinesis/streams", 
               web::delete().to(cloud::kinesis_delete_stream))
        .route("/profiles/{profile}/regions/{region}/kinesis/streams/describe", 
               web::post().to(cloud::kinesis_describe_stream))
        
        // New comprehensive Kinesis operations
        .route("/profiles/{profile}/regions/{region}/kinesis/limits", 
               web::get().to(cloud::kinesis_describe_limits))
        .route("/profiles/{profile}/regions/{region}/kinesis/streams/summary", 
               web::post().to(cloud::kinesis_describe_stream_summary))
        .route("/profiles/{profile}/regions/{region}/kinesis/streams/retention/increase", 
               web::post().to(cloud::kinesis_increase_retention_period))
        .route("/profiles/{profile}/regions/{region}/kinesis/streams/retention/decrease", 
               web::post().to(cloud::kinesis_decrease_retention_period))
        .route("/profiles/{profile}/regions/{region}/kinesis/streams/monitoring/enable", 
               web::post().to(cloud::kinesis_enable_enhanced_monitoring))
        .route("/profiles/{profile}/regions/{region}/kinesis/streams/monitoring/disable", 
               web::post().to(cloud::kinesis_disable_enhanced_monitoring))
        .route("/profiles/{profile}/regions/{region}/kinesis/shards", 
               web::post().to(cloud::kinesis_list_shards))
        
        // Kinesis data plane operations
        .route("/profiles/{profile}/regions/{region}/kinesis/records/put", 
               web::post().to(cloud::kinesis_put_records))
        .route("/profiles/{profile}/regions/{region}/kinesis/records/get", 
               web::post().to(cloud::kinesis_get_records))
        .route("/profiles/{profile}/regions/{region}/kinesis/shard-iterator", 
               web::post().to(cloud::kinesis_get_shard_iterator));
    
    // Register the scopes
    cfg.service(cloud_scope);
    cfg.service(aws_scope);
    cfg.service(aws_data_scope);
}