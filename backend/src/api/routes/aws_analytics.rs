use std::sync::Arc;
use actix_web::web;
use crate::controllers::aws_analytics::AwsAnalyticsController;

pub fn configure(cfg: &mut web::ServiceConfig, aws_analytics_controller: Arc<AwsAnalyticsController>) {
    AwsAnalyticsController::configure(cfg, aws_analytics_controller);
}
