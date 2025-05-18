pub mod auth;
pub mod database;
pub mod kafka;
pub mod kubernetes;
pub mod cloud;
pub mod chaos;
pub mod ai;
pub mod graphql;
pub mod aws_account;

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    auth::configure(cfg);
    database::configure(cfg);
    kafka::configure(cfg);
    kubernetes::configure(cfg);
    cloud::configure(cfg);
    chaos::configure(cfg);
    ai::configure(cfg);
    graphql::configure(cfg);
    aws_account::configure(cfg);
}
