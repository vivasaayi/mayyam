use crate::controllers::kafka;
use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = web::scope("/api/kafka")
        .route("/clusters", web::get().to(kafka::list_clusters))
        .route("/clusters", web::post().to(kafka::create_cluster))
        .route("/clusters/{id}", web::get().to(kafka::get_cluster))
        .route("/clusters/{id}/health", web::get().to(kafka::health_check))
        .route("/metrics", web::get().to(kafka::get_metrics))
        .route(
            "/clusters/{id}/batch-produce",
            web::post().to(kafka::produce_batch),
        )
        .route(
            "/clusters/{id}/produce-retry",
            web::post().to(kafka::produce_with_retry),
        )
        .route("/clusters/{id}/topics", web::get().to(kafka::list_topics))
        .route("/clusters/{id}/topics", web::post().to(kafka::create_topic))
        .route(
            "/clusters/{id}/topics/{topic}",
            web::get().to(kafka::get_topic),
        )
        .route(
            "/clusters/{id}/topics/{topic}",
            web::delete().to(kafka::delete_topic),
        )
        .route(
            "/clusters/{id}/topics/{topic}/produce",
            web::post().to(kafka::produce_message),
        )
        .route(
            "/clusters/{id}/topics/{topic}/consume",
            web::post().to(kafka::consume_messages),
        )
        .route(
            "/clusters/{id}/consumer-groups",
            web::get().to(kafka::list_consumer_groups),
        )
        .route(
            "/clusters/{id}/consumer-groups/{group}",
            web::get().to(kafka::get_consumer_group),
        )
        .route(
            "/clusters/{id}/consumer-groups/{group}/reset",
            web::post().to(kafka::reset_offsets),
        )
        .route(
            "/clusters/{id}/topics/{topic}/config",
            web::put().to(kafka::update_topic_config),
        )
        .route("/clusters/{id}", web::put().to(kafka::update_cluster))
        .route(
            "/clusters/{id}/topics/{topic}/partitions",
            web::post().to(kafka::add_topic_partitions),
        )
        .route(
            "/clusters/{id}/brokers",
            web::get().to(kafka::get_broker_status),
        )
        .route(
            "/clusters/{id}/backup",
            web::post().to(kafka::backup_topic_messages),
        )
        .route(
            "/clusters/{id}/restore",
            web::post().to(kafka::restore_topic_messages),
        )
        .route("/migrate", web::post().to(kafka::migrate_topic_messages))
        .route(
            "/clusters/{id}/drain",
            web::post().to(kafka::wait_for_queue_drain),
        );

    cfg.service(scope);
}
