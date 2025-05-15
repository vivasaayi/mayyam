use actix_web::{web, App, HttpServer, middleware::Logger};
use actix_cors::Cors;
use tracing::{info, error};
use std::error::Error;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

use crate::config::Config;
use crate::api::routes;
use crate::middleware::auth::AuthMiddleware;
use crate::utils::database;
use crate::repositories::{
    user::UserRepository,
    database::DatabaseRepository,
    cluster::ClusterRepository,
    aws_resource::AwsResourceRepository,
};
use crate::services::{
    user::UserService,
    kafka::KafkaService,
    aws::{AwsService, AwsControlPlane, AwsDataPlane, AwsCostService, CloudWatchService},
};
use crate::controllers::{
    auth::AuthController,
    // Import other controllers as needed
};

pub async fn run_server(host: String, port: u16, config: Config) -> Result<(), Box<dyn Error>> {
    let addr = format!("{}:{}", host, port);
    
    info!("Starting Mayyam server on http://{}", addr);
    
    // Connect to the database
    let db_connection = database::connect(&config).await?;
    
    // Initialize repositories
    let user_repo = Arc::new(UserRepository::new(db_connection.clone()));
    let database_repo = Arc::new(DatabaseRepository::new(db_connection.clone(), config.clone()));
    let cluster_repo = Arc::new(ClusterRepository::new(db_connection.clone(), config.clone()));
    let aws_resource_repo = Arc::new(AwsResourceRepository::new(db_connection.clone(), config.clone()));
    
    // Initialize services
    let user_service = Arc::new(UserService::new(user_repo.clone()));
    let kafka_service = Arc::new(KafkaService::new(cluster_repo.clone()));
    
    // AWS services
    let aws_service = Arc::new(AwsService::new(aws_resource_repo.clone(), config.clone()));
    let aws_control_plane = Arc::new(AwsControlPlane::new(aws_service.clone()));
    let aws_data_plane = Arc::new(AwsDataPlane::new(aws_service.clone()));
    let aws_cost_service = Arc::new(AwsCostService::new(aws_service.clone()));
    let cloudwatch_service = Arc::new(CloudWatchService::new(aws_service.clone()));
    
    // Initialize controllers
    let auth_controller = Arc::new(AuthController::new(user_service.clone(), config.clone()));
    // Initialize other controllers here
    
    // Create and start the HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
            
        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            .wrap(AuthMiddleware::new(&config))
            .app_data(web::Data::new(db_connection.clone()))
            .app_data(web::Data::new(config.clone()))
            // Repositories
            .app_data(web::Data::new(user_repo.clone()))
            .app_data(web::Data::new(database_repo.clone()))
            .app_data(web::Data::new(cluster_repo.clone()))
            .app_data(web::Data::new(aws_resource_repo.clone()))
            // Services
            .app_data(web::Data::new(user_service.clone()))
            .app_data(web::Data::new(kafka_service.clone()))
            .app_data(web::Data::new(aws_service.clone()))
            .app_data(web::Data::new(aws_control_plane.clone()))
            .app_data(web::Data::new(aws_data_plane.clone()))
            .app_data(web::Data::new(aws_cost_service.clone()))
            .app_data(web::Data::new(cloudwatch_service.clone()))
            // Controllers
            .app_data(web::Data::new(auth_controller.clone()))
            // Routes configuration
            .configure(routes::configure)
            .service(web::resource("/health").to(|| async { "Mayyam API is running!" }))
    })
    .bind(addr)?
    .run()
    .await?;
    
    Ok(())
}
