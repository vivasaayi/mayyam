use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/kubernetes")
            .route("/clusters", web::get().to(|| async { "List Kubernetes clusters" }))
            .route("/clusters", web::post().to(|| async { "Add Kubernetes cluster" }))
            .route("/clusters/{id}", web::get().to(|| async { "Get cluster info" }))
            .route("/clusters/{id}/namespaces", web::get().to(|| async { "List namespaces" }))
            .route("/clusters/{id}/pods", web::get().to(|| async { "List pods" }))
            .route("/clusters/{id}/pods/{namespace}/{name}", web::get().to(|| async { "Get pod details" }))
            .route("/clusters/{id}/pods/{namespace}/{name}/logs", web::get().to(|| async { "Get pod logs" }))
            .route("/clusters/{id}/deployments", web::get().to(|| async { "List deployments" }))
            .route("/clusters/{id}/deployments/{namespace}/{name}", web::get().to(|| async { "Get deployment details" }))
            .route("/clusters/{id}/deployments/{namespace}/{name}/scale", web::post().to(|| async { "Scale deployment" }))
            .route("/clusters/{id}/services", web::get().to(|| async { "List services" }))
            .route("/clusters/{id}/nodes", web::get().to(|| async { "List nodes" }))
            .route("/clusters/{id}/events", web::get().to(|| async { "List events" }))
            .route("/clusters/{id}/metrics", web::get().to(|| async { "Get cluster metrics" }))
            .route("/clusters/{id}/resource-usage", web::get().to(|| async { "Get resource usage" }))
    );
}
