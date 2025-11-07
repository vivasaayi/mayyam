use crate::controllers::ai;
use crate::middleware::auth::Claims;
use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = web::scope("/api/ai")
        .route("/analyze", web::post().to(analyze_data))
        .route("/generate", web::post().to(generate_content))
        .route("/summary", web::post().to(generate_summary))
        .route("/explain", web::post().to(explain_data))
        .route("/chat", web::post().to(ai::chat))
        .route("/chat/stream", web::post().to(ai::chat_stream))
        .route(
            "/analyze/rds/{id}/{workflow}",
            web::get().to(ai::analyze_rds_instance),
        )
        .route(
            "/analyze/rds/question",
            web::post().to(ai::answer_rds_question),
        );

    cfg.service(scope);
}

async fn analyze_data() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "This endpoint will analyze data using AI",
        "analysis": {
            "trends": [
                {
                    "name": "Increasing latency",
                    "confidence": 0.95,
                    "details": "System latency has increased by 23% over the last hour"
                },
                {
                    "name": "Memory usage spike",
                    "confidence": 0.87,
                    "details": "Memory usage spiked at 2:15 PM"
                }
            ],
            "recommendations": [
                "Investigate database query performance",
                "Consider scaling up the service"
            ]
        }
    }))
}

async fn generate_content() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "This endpoint will generate content using AI",
        "content": "Here is an example of AI-generated content based on your prompt."
    }))
}

async fn generate_summary() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "This endpoint will generate a summary of data using AI",
        "summary": "This system experienced increased load during peak hours (9AM-11AM). There were 3 error spikes during this period, with the longest lasting 12 minutes. Overall system health is good with 99.95% availability."
    }))
}

async fn explain_data() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "This endpoint will explain complex data using AI",
        "explanation": "The database latency increased because of a blocking query that was initiated at 10:15 AM. This query was accessing a table without proper indexing, causing full table scans. The issue resolved after the query completed at 10:27 AM."
    }))
}
