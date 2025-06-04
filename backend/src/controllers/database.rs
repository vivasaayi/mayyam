use std::sync::Arc;
use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;

use crate::config::Config;
use crate::errors::AppError;
use crate::models::database::{
    DatabaseQueryRequest, CreateDatabaseConnectionRequest
};
use crate::services::database::DatabaseService;
use crate::repositories::database::DatabaseRepository;
use crate::middleware::auth::Claims;
use crate::services::analytics::mysql_analytics::mysql_analytics_service::MySqlAnalyticsService;
use crate::services::analytics::postgres_analytics::postgres_analytics_service::PostgresAnalyticsService;

pub async fn execute_query(
    query_req: web::Json<DatabaseQueryRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // TODO: Update this method to handle postgresql and mysql
    let db_service = MySqlAnalyticsService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn_id = uuid::Uuid::parse_str(&query_req.connection_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let conn = db_repo.find_by_id(conn_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Database connection not found: {}", query_req.connection_id)))?;

    // Execute the query with analysis if requested
    let result = if query_req.explain.unwrap_or(false) {
        db_service.execute_query_with_explain(&conn, &query_req.query, query_req.params.as_ref()).await?
    } else {
        db_service.execute_query(&conn, &query_req.query, query_req.params.as_ref()).await?
    };

    Ok(HttpResponse::Ok().json(result))
}

pub async fn analyze_database(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // ToDo: Update this method to handle postgresql and mysql
    let db_service = PostgresAnalyticsService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details to check if it exists
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let conn_model = db_repo.find_by_id(conn_id).await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;
    
    // Log that we're analyzing the connection for debugging purposes
    tracing::info!("Analyzing database connection: {}", conn_model.name);

    // Use the new analyze_connection method
    let analysis = db_service.analyze_connection(&conn_model).await?;

    Ok(HttpResponse::Ok().json(analysis))
}

pub async fn list_connections(
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let connections = db_repo.find_all().await?;
    
    Ok(HttpResponse::Ok().json(connections))
}

pub async fn get_connection(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let connection = db_repo.find_by_id(conn_id).await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;
    
    Ok(HttpResponse::Ok().json(connection))
}

pub async fn create_connection(
    connection: web::Json<CreateDatabaseConnectionRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Create the database connection
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let new_connection = db_repo.create(&connection, user_id).await?;
    
    Ok(HttpResponse::Created().json(new_connection))
}

pub async fn update_connection(
    path: web::Path<String>,
    connection: web::Json<CreateDatabaseConnectionRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Update the database connection
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let updated_connection = db_repo.update(conn_id, &connection).await?;
    
    Ok(HttpResponse::Ok().json(updated_connection))
}

pub async fn delete_connection(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Delete the database connection
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    db_repo.delete(conn_id).await?;
    
    Ok(HttpResponse::NoContent().finish())
}

pub async fn test_connection(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_service = DatabaseService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let conn = db_repo.find_by_id(conn_id).await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    // Test the connection
    let test_result = db_service.test_connection(&conn).await?;

    Ok(HttpResponse::Ok().json(test_result))
}

pub async fn get_schema(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_service = DatabaseService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let conn = db_repo.find_by_id(conn_id).await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    // Get the database schema
    let schema = db_service.get_schema(&conn).await?;

    Ok(HttpResponse::Ok().json(schema))
}
