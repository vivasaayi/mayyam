use actix_web::{web, App, HttpServer, HttpResponse, Responder, middleware::Logger, http::header};
use actix_cors::Cors;
use serde_json::{self, json};
use std::io;
use std::collections::HashMap;

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Mayyam API Server")
}

async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "up",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn list_k8s_clusters() -> impl Responder {
    // Mock data for demo
    HttpResponse::Ok().json(serde_json::json!([
        {
            "name": "dev-cluster",
            "context": "kubernetes-dev",
            "version": "1.25.5",
            "status": "active",
            "nodes": 3
        },
        {
            "name": "prod-cluster",
            "context": "kubernetes-prod",
            "version": "1.24.8",
            "status": "active",
            "nodes": 5
        }
    ]))
}

async fn list_ec2_instances() -> impl Responder {
    // Mock data for demo
    HttpResponse::Ok().json(serde_json::json!([
        {
            "id": "i-0abc12345def67890",
            "name": "web-server-1",
            "type": "t3.medium",
            "state": "running",
            "region": "us-west-2",
            "tags": {
                "Environment": "production",
                "Service": "web"
            }
        },
        {
            "id": "i-0123abcdef456789",
            "name": "api-server-1",
            "type": "t3.large",
            "state": "running", 
            "region": "us-west-2",
            "tags": {
                "Environment": "production",
                "Service": "api"
            }
        }
    ]))
}

#[derive(serde::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn login(login_data: web::Json<LoginRequest>) -> impl Responder {
    // In a real app, we would verify credentials against a database
    // and generate a proper JWT token.
    if login_data.username == "admin" && login_data.password == "admin" {
        HttpResponse::Ok().json(json!({
            "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyLCJleHAiOjk5OTk5OTk5OTl9.KcZioVJUMnhw7MEE-nFnqYlJnr3GU9fDlDos9VRO9Sg",
            "user": {
                "id": "1",
                "username": "admin",
                "email": "admin@example.com",
                "firstName": "Admin",
                "lastName": "User",
                "roles": ["admin"]
            }
        }))
    } else {
        HttpResponse::Unauthorized().json(json!({
            "message": "Invalid credentials"
        }))
    }
}

async fn register(user_data: web::Json<serde_json::Value>) -> impl Responder {
    // In a real app, we would save the user to a database
    HttpResponse::Created().json(json!({
        "message": "User registered successfully",
        "userId": "new-user-id"
    }))
}

async fn list_aws_resources() -> impl Responder {
    // Mock AWS resources data
    let resources = vec![
        json!({
            "resource_id": "i-0abc12345def67890",
            "resource_type": "EC2Instance",
            "name": "web-server-1",
            "account_id": "123456789012",
            "region": "us-west-2",
            "arn": "arn:aws:ec2:us-west-2:123456789012:instance/i-0abc12345def67890",
            "created_at": "2025-01-01T00:00:00Z",
            "tags": {
                "Environment": "production",
                "Service": "web"
            }
        }),
        json!({
            "resource_id": "i-0123abcdef456789",
            "resource_type": "EC2Instance",
            "name": "api-server-1",
            "account_id": "123456789012",
            "region": "us-west-2",
            "arn": "arn:aws:ec2:us-west-2:123456789012:instance/i-0123abcdef456789",
            "created_at": "2025-01-01T00:00:00Z",
            "tags": {
                "Environment": "production",
                "Service": "api"
            }
        }),
        json!({
            "resource_id": "my-bucket",
            "resource_type": "S3Bucket",
            "name": "my-bucket",
            "account_id": "123456789012",
            "region": "us-east-1",
            "arn": "arn:aws:s3:::my-bucket",
            "created_at": "2025-01-01T00:00:00Z",
            "tags": {
                "Environment": "production",
                "Purpose": "storage"
            }
        }),
        json!({
            "resource_id": "database-1",
            "resource_type": "RdsInstance",
            "name": "database-1",
            "account_id": "123456789012",
            "region": "us-west-2",
            "arn": "arn:aws:rds:us-west-2:123456789012:db:database-1",
            "created_at": "2025-01-01T00:00:00Z",
            "tags": {
                "Environment": "production",
                "Service": "database"
            }
        })
    ];

    HttpResponse::Ok().json(json!({
        "resources": resources,
        "total": resources.len()
    }))
}

async fn list_k8s_namespaces() -> impl Responder {
    // Mock Kubernetes namespaces
    HttpResponse::Ok().json(json!([
        {
            "name": "default",
            "status": "active",
            "age": "10d",
            "labels": {
                "kubernetes.io/metadata.name": "default"
            }
        },
        {
            "name": "kube-system",
            "status": "active",
            "age": "10d",
            "labels": {
                "kubernetes.io/metadata.name": "kube-system"
            }
        },
        {
            "name": "app",
            "status": "active",
            "age": "2d",
            "labels": {
                "environment": "production"
            }
        }
    ]))
}

async fn list_k8s_pods() -> impl Responder {
    // Mock Kubernetes pods
    HttpResponse::Ok().json(json!([
        {
            "name": "web-app-7b9c8d6f5-abcde",
            "namespace": "app",
            "status": "Running",
            "age": "1d",
            "ip": "10.0.0.1",
            "node": "worker-1"
        },
        {
            "name": "api-app-7b9c8d6f5-fghij",
            "namespace": "app",
            "status": "Running",
            "age": "1d",
            "ip": "10.0.0.2",
            "node": "worker-2"
        },
        {
            "name": "database-5b6c7d8e9-klmno",
            "namespace": "app",
            "status": "Running",
            "age": "1d",
            "ip": "10.0.0.3",
            "node": "worker-1"
        }
    ]))
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    println!("Starting Mayyam API server on http://127.0.0.1:8080");
    
    HttpServer::new(|| {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
        
        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health))
            
            // Kubernetes endpoints
            .route("/api/kubernetes/clusters", web::get().to(list_k8s_clusters))
            .route("/api/kubernetes/namespaces", web::get().to(list_k8s_namespaces))
            .route("/api/kubernetes/pods", web::get().to(list_k8s_pods))
            
            // AWS endpoints
            .route("/api/aws/ec2/instances", web::get().to(list_ec2_instances))
            .route("/api/aws/resources", web::get().to(list_aws_resources))
            
            // Auth endpoints
            .route("/api/auth/login", web::post().to(login))
            .route("/api/auth/register", web::post().to(register))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
