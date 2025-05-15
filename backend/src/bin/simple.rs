use actix_web::{web, App, HttpServer, HttpResponse, Responder, middleware::Logger};
use actix_cors::Cors;
use serde_json;
use std::io;

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
            .route("/api/kubernetes/clusters", web::get().to(list_k8s_clusters))
            .route("/api/aws/ec2/instances", web::get().to(list_ec2_instances))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
