use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tracing::{info, warn, error};

use crate::config::Config;
use crate::errors::AppError;
use crate::models::database::{
    DatabaseQueryRequest, DatabaseQueryResponse, DatabaseAnalysis,
    CreateDatabaseConnectionRequest, ConnectionTestResult
};
use crate::services::database::DatabaseService;
use crate::repositories::database::DatabaseRepository;
use crate::middleware::auth::Claims;

pub async fn execute_query(
    query_req: web::Json<DatabaseQueryRequest>,
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_service = DatabaseService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn = db_repo.find_by_id(&query_req.connection_id).await?
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
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_service = DatabaseService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn = db_repo.find_by_id(&path.into_inner()).await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    // Perform comprehensive database analysis
    let analysis = db_service.analyze_database(db_pool.as_ref()).await?;

    Ok(HttpResponse::Ok().json(analysis))
}

pub async fn list_connections(
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let connections = db_repo.find_all().await?;
    
    Ok(HttpResponse::Ok().json(connections))
}

pub async fn get_connection(
    path: web::Path<String>,
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    let connection = db_repo.find_by_id(&path.into_inner()).await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;
    
    Ok(HttpResponse::Ok().json(connection))
}

pub async fn create_connection(
    connection: web::Json<CreateDatabaseConnectionRequest>,
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Create the database connection
    let new_connection = db_repo.create(&connection, &claims.sub).await?;
    
    Ok(HttpResponse::Created().json(new_connection))
}

pub async fn update_connection(
    path: web::Path<String>,
    connection: web::Json<CreateDatabaseConnectionRequest>,
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Update the database connection
    let updated_connection = db_repo.update(&path.into_inner(), &connection).await?;
    
    Ok(HttpResponse::Ok().json(updated_connection))
}

pub async fn delete_connection(
    path: web::Path<String>,
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Delete the database connection
    db_repo.delete(&path.into_inner()).await?;
    
    Ok(HttpResponse::NoContent().finish())
}

pub async fn test_connection(
    path: web::Path<String>,
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_service = DatabaseService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn = db_repo.find_by_id(&path.into_inner()).await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    // Test the connection
    let test_result = db_service.test_connection(&conn).await?;

    Ok(HttpResponse::Ok().json(test_result))
}

pub async fn get_schema(
    path: web::Path<String>,
    db_pool: web::Data<DatabaseConnection>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_service = DatabaseService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn = db_repo.find_by_id(&path.into_inner()).await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    // Get the database schema
    let schema = db_service.get_schema(&conn).await?;

    Ok(HttpResponse::Ok().json(schema))
}
