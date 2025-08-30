pub mod auth;
pub mod database;
pub mod kafka;
pub mod kubernetes;
pub mod cloud;
pub mod chaos;
pub mod ai;
pub mod graphql;
pub mod aws_account;
pub mod aws_analytics;
pub mod kubernetes_cluster_management; // New module
pub mod data_source;
pub mod llm_provider;
pub mod prompt_template;
pub mod query_template;
pub mod llm_analytics;

use actix_web::web;
use sea_orm::DatabaseConnection; // Ensure this is imported
use std::sync::Arc; // Ensure this is imported

// Modified signature to accept db connection
pub fn configure(cfg: &mut web::ServiceConfig, db: Arc<DatabaseConnection>) {
    auth::configure(cfg);
    database::configure(cfg); // This might also need the db if it configures routes needing it directly
    kafka::configure(cfg);    // Same for this
    kubernetes::configure(cfg, db.clone()); // Pass db to kubernetes::configure
    cloud::configure(cfg);
    chaos::configure(cfg);
    ai::configure(cfg);
    graphql::configure(cfg);
    // Note: aws_account and aws_analytics are configured separately
    // with dependency injection in server.rs to avoid route conflicts
    // DO NOT configure aws_analytics here
}
