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
               web::post().to(cloud::kinesis_put_record));
    
    // Register the scopes
    cfg.service(cloud_scope);
    cfg.service(aws_scope);
    cfg.service(aws_data_scope);
}

async fn list_connections() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "This endpoint will list all cloud provider connections",
        "connections": []
    }))
}

async fn create_connection() -> HttpResponse {
    HttpResponse::Created().json(serde_json::json!({
        "message": "This endpoint will create a new cloud provider connection",
        "success": true
    }))
}

async fn get_connection(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will fetch cloud provider connection with ID: {}", id),
        "id": id
    }))
}

async fn update_connection(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will update cloud provider connection with ID: {}", id),
        "id": id,
        "success": true
    }))
}

async fn delete_connection(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will delete cloud provider connection with ID: {}", id),
        "id": id,
        "success": true
    }))
}

// AWS-specific handlers
async fn list_aws_regions(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will list AWS regions for connection ID: {}", id),
        "id": id,
        "regions": [
            {
                "region_name": "us-east-1",
                "region_description": "US East (N. Virginia)"
            },
            {
                "region_name": "us-west-2",
                "region_description": "US West (Oregon)"
            }
        ]
    }))
}

async fn list_aws_ec2(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will list AWS EC2 instances for connection ID: {}", id),
        "id": id,
        "instances": [
            {
                "instance_id": "i-0123456789abcdef0",
                "instance_type": "t2.micro",
                "state": "running",
                "public_ip": "203.0.113.1",
                "private_ip": "10.0.0.1"
            }
        ]
    }))
}

async fn list_aws_s3(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will list AWS S3 buckets for connection ID: {}", id),
        "id": id,
        "buckets": [
            {
                "name": "example-bucket",
                "creation_date": "2025-01-01T00:00:00Z",
                "region": "us-east-1"
            }
        ]
    }))
}

async fn list_aws_rds(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will list AWS RDS instances for connection ID: {}", id),
        "id": id,
        "instances": [
            {
                "identifier": "database-1",
                "engine": "postgres",
                "status": "available",
                "endpoint": "database-1.abcdefgh.us-east-1.rds.amazonaws.com",
                "port": 5432
            }
        ]
    }))
}

// Azure-specific handlers
async fn list_azure_regions(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will list Azure regions for connection ID: {}", id),
        "id": id,
        "regions": [
            {
                "name": "eastus",
                "display_name": "East US"
            },
            {
                "name": "westeurope",
                "display_name": "West Europe"
            }
        ]
    }))
}

async fn list_azure_vms(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will list Azure VMs for connection ID: {}", id),
        "id": id,
        "virtual_machines": [
            {
                "name": "vm-example",
                "resource_group": "example-resources",
                "location": "eastus",
                "vm_size": "Standard_B2s",
                "status": "running",
                "private_ip": "10.0.0.4",
                "public_ip": "203.0.113.10"
            }
        ]
    }))
}

async fn list_azure_storage(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will list Azure Storage accounts for connection ID: {}", id),
        "id": id,
        "storage_accounts": [
            {
                "name": "examplestorage",
                "resource_group": "example-resources",
                "location": "eastus",
                "sku": "Standard_LRS",
                "kind": "StorageV2"
            }
        ]
    }))
}
