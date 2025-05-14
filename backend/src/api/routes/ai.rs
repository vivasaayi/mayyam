use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/ai")
            .route("/chat", web::post().to(|| async { "Chat with AI assistant" }))
            .route("/analyze-logs", web::post().to(|| async { "Analyze logs with AI" }))
            .route("/analyze-metrics", web::post().to(|| async { "Analyze metrics with AI" }))
            .route("/optimize-query", web::post().to(|| async { "Optimize SQL query with AI" }))
            .route("/explain-kubernetes", web::post().to(|| async { "Explain Kubernetes resources with AI" }))
            .route("/troubleshoot", web::post().to(|| async { "AI-assisted troubleshooting" }))
    );
}
