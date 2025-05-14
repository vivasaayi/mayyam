use actix_web::web;
use serde::{Deserialize, Serialize};
use crate::controllers::database;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionRequest {
    pub db_type: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    pub connection_id: String,
    pub query: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/database")
            .route("/connections", web::get().to(database::list_connections))
            .route("/connections", web::post().to(database::create_connection))
            .route("/connections/{id}", web::get().to(database::get_connection))
            .route("/connections/{id}", web::delete().to(database::delete_connection))
            .route("/query", web::post().to(database::execute_query))
            .route("/schema/{id}", web::get().to(database::get_schema))
            .route("/analyze/{id}", web::get().to(database::analyze_database))
    );
}
