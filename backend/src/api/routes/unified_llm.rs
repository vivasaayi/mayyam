use actix_web::{web, HttpResponse, Result};
use std::sync::Arc;

use crate::controllers::unified_llm::UnifiedLlmController;

pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<UnifiedLlmController>) {
    cfg.service(
        web::scope("/api/llm")
            .app_data(web::Data::new(controller))
            // Generate response with specified provider
            .route("/generate", web::post().to(generate))
            // Smart generation - automatically selects best provider
            .route("/generate/smart", web::post().to(generate_smart))
            // Quick generation for simple use cases
            .route("/generate/quick", web::post().to(quick_generate))
            // List available providers
            .route("/providers", web::get().to(list_providers))
            // Get provider capabilities
            .route(
                "/providers/{provider}/capabilities",
                web::get().to(get_provider_capabilities),
            )
            // Estimate costs for a request across all providers
            .route("/estimate-costs", web::post().to(estimate_costs)),
    );
}

async fn generate(
    controller: web::Data<Arc<UnifiedLlmController>>,
    request: web::Json<crate::controllers::unified_llm::SimpleGenerationRequest>,
) -> Result<HttpResponse> {
    controller.generate(request).await
}

async fn generate_smart(
    controller: web::Data<Arc<UnifiedLlmController>>,
    request: web::Json<crate::controllers::unified_llm::SmartGenerationRequest>,
) -> Result<HttpResponse> {
    controller.generate_smart(request).await
}

async fn quick_generate(
    controller: web::Data<Arc<UnifiedLlmController>>,
    request: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    controller.quick_generate(request).await
}

async fn list_providers(controller: web::Data<Arc<UnifiedLlmController>>) -> Result<HttpResponse> {
    controller.list_providers().await
}

async fn get_provider_capabilities(
    controller: web::Data<Arc<UnifiedLlmController>>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    controller.get_provider_capabilities(path).await
}

async fn estimate_costs(
    controller: web::Data<Arc<UnifiedLlmController>>,
    request: web::Json<crate::controllers::unified_llm::SmartGenerationRequest>,
) -> Result<HttpResponse> {
    controller.estimate_costs(request).await
}
