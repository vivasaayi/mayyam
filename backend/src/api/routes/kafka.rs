use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/kafka")
            .route("/clusters", web::get().to(|| async { "List Kafka clusters" }))
            .route("/clusters", web::post().to(|| async { "Create Kafka cluster connection" }))
            .route("/clusters/{id}", web::get().to(|| async { "Get Kafka cluster" }))
            .route("/clusters/{id}/topics", web::get().to(|| async { "List topics" }))
            .route("/clusters/{id}/topics", web::post().to(|| async { "Create topic" }))
            .route("/clusters/{id}/topics/{topic}", web::get().to(|| async { "Get topic details" }))
            .route("/clusters/{id}/topics/{topic}", web::delete().to(|| async { "Delete topic" }))
            .route("/clusters/{id}/topics/{topic}/messages", web::get().to(|| async { "Consume messages" }))
            .route("/clusters/{id}/topics/{topic}/messages", web::post().to(|| async { "Produce message" }))
            .route("/clusters/{id}/consumer-groups", web::get().to(|| async { "List consumer groups" }))
            .route("/clusters/{id}/consumer-groups/{group}", web::get().to(|| async { "Get consumer group details" }))
            .route("/clusters/{id}/consumer-groups/{group}/offsets", web::post().to(|| async { "Reset consumer group offsets" }))
    );
}
