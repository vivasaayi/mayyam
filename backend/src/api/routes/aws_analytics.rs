use crate::controllers::aws_analytics::AwsAnalyticsController;
use actix_web::web;
use std::sync::Arc;

pub fn configure(
    cfg: &mut web::ServiceConfig,
    aws_analytics_controller: Arc<AwsAnalyticsController>,
) {
    AwsAnalyticsController::configure(cfg, aws_analytics_controller);
}
