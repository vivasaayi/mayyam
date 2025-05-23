use actix_web::{middleware::Logger, web, App, HttpServer};
use actix_cors::Cors;
use tracing::{error, info};
use std::error::Error;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

use crate::config::Config;
use crate::api::routes;
use crate::middleware::auth::AuthMiddleware;
use crate::services::aws::aws_control_plane::s3_control_plane::S3ControlPlane;
use crate::utils::database;
use crate::repositories::{
    aws_account::AwsAccountRepository,
    aws_resource::AwsResourceRepository,
    cluster::ClusterRepository,
    database::DatabaseRepository,
    user::UserRepository,
};
use crate::services::{
    aws::{AwsControlPlane, AwsCostService, AwsDataPlane, AwsService},
    aws_account::AwsAccountService,
    kafka::KafkaService,
    user::UserService,
};
use crate::controllers::{
    auth::AuthController,
    aws_analytics::AwsAnalyticsController,
    // Import other controllers as needed
};
use crate::services::analytics::aws_analytics::aws_analytics::AwsAnalyticsService;
use crate::services::aws::aws_control_plane::dynamodb_control_plane::DynamoDbControlPlane;
use crate::services::aws::aws_control_plane::kinesis_control_plane::KinesisControlPlane;
use crate::services::aws::aws_control_plane::s3_control_plane;
use crate::services::aws::aws_control_plane::sqs_control_plane::SqsControlPlane;
use crate::services::aws::aws_data_plane::cloudwatch::CloudWatchService;
use crate::services::aws::aws_data_plane::dynamodb_data_plane::DynamoDBDataPlane;
use crate::services::aws::aws_data_plane::kinesis_data_plane::KinesisDataPlane;
use crate::services::aws::aws_data_plane::s3_data_plane::S3DataPlane;
use crate::services::aws::aws_data_plane::sqs_data_plane::SqsDataPlane;

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
    let aws_account_repo = Arc::new(AwsAccountRepository::new(db_connection.clone()));
    
    // Initialize services
    let user_service = Arc::new(UserService::new(user_repo.clone()));
    let kafka_service = Arc::new(KafkaService::new(cluster_repo.clone()));
    
    // AWS services
    let aws_service = Arc::new(AwsService::new(aws_resource_repo.clone(), config.clone()));
    let aws_control_plane = Arc::new(AwsControlPlane::new(aws_service.clone()));
    let aws_data_plane = Arc::new(AwsDataPlane::new(aws_service.clone()));
    let aws_cost_service = Arc::new(AwsCostService::new(aws_service.clone()));
    let cloudwatch_service = Arc::new(CloudWatchService::new(aws_service.clone()));
    let aws_account_service = Arc::new(AwsAccountService::new(aws_account_repo.clone(), aws_control_plane.clone()));
    let aws_analytics_service = Arc::new(AwsAnalyticsService::new(
        config.clone(),
        aws_service.clone(),
        aws_data_plane.clone(),
        aws_resource_repo.clone(),
    ));
    
    // Initialize controllers
    let auth_controller = Arc::new(AuthController::new(user_service.clone(), config.clone()));
    let aws_analytics_controller = Arc::new(AwsAnalyticsController::new(aws_analytics_service.clone()));
    // Initialize other controllers here

    let s3_data_plane = Arc::new(S3DataPlane::new(aws_service.clone()));
    let s3_control_plane = Arc::new(s3_control_plane::S3ControlPlane::new(aws_service.clone()));

    let dynamodb_data_plane = Arc::new(DynamoDBDataPlane::new(aws_service.clone()));
    let dynamodb_control_plane = Arc::new(DynamoDbControlPlane::new(aws_service.clone()));

    let sqs_data_plane = Arc::new(SqsDataPlane::new(aws_service.clone()));
    let sqs_control_plane = Arc::new(SqsControlPlane::new(aws_service.clone()));

    let kinesis_data_plane = Arc::new(KinesisDataPlane::new(aws_service.clone()));
    let kinesis_control_plane = Arc::new(KinesisControlPlane::new(aws_service.clone()));

    
    // Create and start the HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .supports_credentials()
            .max_age(3600);
            
        info!("Configuring CORS with credentials support");
            
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
            .app_data(web::Data::new(aws_account_repo.clone()))
            // Services
            .app_data(web::Data::new(user_service.clone()))
            .app_data(web::Data::new(kafka_service.clone()))
            .app_data(web::Data::new(aws_service.clone()))
            .app_data(web::Data::new(aws_control_plane.clone()))
            .app_data(web::Data::new(aws_data_plane.clone()))
            .app_data(web::Data::new(aws_cost_service.clone()))
            .app_data(web::Data::new(cloudwatch_service.clone()))
            .app_data(web::Data::new(aws_account_service.clone()))
            .app_data(web::Data::new(aws_analytics_service.clone()))
            // Controllers
            .app_data(web::Data::new(auth_controller.clone()))
            .app_data(web::Data::new(aws_analytics_controller.clone()))
            .app_data(web::Data::new(s3_data_plane.clone()))
            .app_data(web::Data::new(s3_control_plane.clone()))
            .app_data(web::Data::new(dynamodb_data_plane.clone()))
            .app_data(web::Data::new(dynamodb_control_plane.clone()))
            .app_data(web::Data::new(sqs_data_plane.clone()))
            .app_data(web::Data::new(sqs_control_plane.clone()))
            .app_data(web::Data::new(kinesis_data_plane.clone()))
            .app_data(web::Data::new(kinesis_control_plane.clone()))
            // Middleware
            // Routes configuration - specify the order: analytics first, then general routes
            .configure(|cfg| {
                info!("Registering route handlers in server.rs");
                
                // Explicitly register AWS analytics routes first to avoid route conflicts
                info!("Registering AWS analytics routes with highest priority");
                routes::aws_analytics::configure(cfg, aws_analytics_controller.clone());
                
                // Skip aws_analytics configuration in the general routes to avoid duplicate route registrations
                info!("Registering general routes");
                routes::configure(cfg);
            })
            .service(web::resource("/health").to(|| async { "Mayyam API is running!" }))
    })
    .bind(addr)?
    .run()
    .await?;
    
    Ok(())
}
