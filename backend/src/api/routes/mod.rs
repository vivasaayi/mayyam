// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


pub mod ai;
pub mod auth;
pub mod aws_account;
pub mod aws_analytics;
pub mod budget;
pub mod chaos;
pub mod cloud;
pub mod cost_analytics;
pub mod data_source;
pub mod database;
pub mod explain_plan;
pub mod graphql;
pub mod kafka;
pub mod kubernetes;
pub mod kubernetes_cluster_management; // New module
pub mod llm_analytics;
pub mod llm_provider;
pub mod metrics;
pub mod prompt_template;
pub mod query_fingerprint;
pub mod query_template;
pub mod slow_query;
pub mod sync_run;
pub mod unified_llm;

use actix_web::web;
use sea_orm::DatabaseConnection; // Ensure this is imported
use std::sync::Arc; // Ensure this is imported

// Modified signature to accept db connection
pub fn configure(cfg: &mut web::ServiceConfig, db: Arc<DatabaseConnection>) {
    auth::configure(cfg);
    database::configure(cfg); // This might also need the db if it configures routes needing it directly
    slow_query::configure(cfg, db.clone());
    query_fingerprint::configure(cfg, db.clone());
    explain_plan::configure(cfg, db.clone());
    kafka::configure(cfg); // Same for this
    kubernetes::configure(cfg, db.clone()); // Pass db to kubernetes::configure
    cloud::configure(cfg);
    chaos::configure(cfg);
    ai::configure(cfg);
    graphql::configure(cfg);
    // Note: sync_run routes are registered in server.rs where controller is available
    // Note: aws_account and aws_analytics are configured separately
    // with dependency injection in server.rs to avoid route conflicts
    // DO NOT configure aws_analytics here
}
