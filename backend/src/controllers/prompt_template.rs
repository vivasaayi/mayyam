use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::models::prompt_template::{
    CreatePromptTemplateDto, PromptCategory, PromptStatus, PromptTemplateResponseDto, PromptType,
    UpdatePromptTemplateDto,
};
use crate::repositories::prompt_template::PromptTemplateRepository;

#[derive(Debug, Deserialize)]
pub struct CreatePromptTemplateRequest {
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub prompt_type: String,
    pub template_content: String,
    pub variables: Option<Value>,
    pub tags: Option<Vec<String>>,
    pub is_system_prompt: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePromptTemplateRequest {
    pub description: Option<Option<String>>,
    pub template_content: Option<String>,
    pub variables: Option<Option<Value>>,
    pub tags: Option<Option<Vec<String>>>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVersionRequest {
    pub template_content: String,
    pub variables: Option<Value>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PromptTemplateQueryParams {
    pub category: Option<String>,
    pub prompt_type: Option<String>,
    pub tags: Option<String>, // Comma-separated tags
    pub system_only: Option<bool>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PromptTemplateListResponse {
    pub templates: Vec<PromptTemplateResponseDto>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct PromptVersionsResponse {
    pub versions: Vec<PromptTemplateResponseDto>,
    pub latest_version: i32,
}

#[derive(Debug, Serialize)]
pub struct PopularPromptsResponse {
    pub templates: Vec<PromptTemplateResponseDto>,
}

#[derive(Debug, Deserialize)]
pub struct RenderPromptRequest {
    pub variables: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct RenderPromptResponse {
    pub rendered_content: String,
    pub variables_used: Vec<String>,
}

pub struct PromptTemplateController {
    prompt_template_repo: Arc<PromptTemplateRepository>,
}

impl PromptTemplateController {
    pub fn new(prompt_template_repo: Arc<PromptTemplateRepository>) -> Self {
        Self {
            prompt_template_repo,
        }
    }

    pub async fn create_prompt_template(
        controller: web::Data<PromptTemplateController>,
        request: web::Json<CreatePromptTemplateRequest>,
    ) -> ActixResult<HttpResponse> {
        let dto = CreatePromptTemplateDto {
            name: request.name.clone(),
            category: PromptCategory::from(request.category.clone()),
            resource_type: None,
            workflow_type: None,
            prompt_template: request.template_content.clone(),
            variables: vec![], // TODO: parse variables if needed
            version: None,
            is_active: Some(true),
            is_system: Some(request.is_system_prompt),
            description: request.description.clone(),
            tags: request.tags.clone().unwrap_or_default(),
            prompt_type: Some(match request.prompt_type.as_str() {
                "System" => PromptType::System,
                "User" => PromptType::User,
                "Analysis" => PromptType::Analysis,
                "Workflow" => PromptType::Workflow,
                _ => PromptType::User,
            }),
            template_content: Some(request.template_content.clone()),
            is_system_prompt: Some(request.is_system_prompt),
            parent_id: None,
            status: Some(PromptStatus::Active), // Default to Active or set as needed
            // Remove status field if not present in request
            usage_count: None,
            last_used_at: None,
        };

        let template = controller
            .prompt_template_repo
            .create(dto)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

        Ok(HttpResponse::Ok().json(PromptTemplateResponseDto::from(template)))
    }

    pub async fn get_prompt_template(
        controller: web::Data<PromptTemplateController>,
        path: web::Path<Uuid>,
    ) -> ActixResult<HttpResponse> {
        let template = controller
            .prompt_template_repo
            .find_by_id(&*path)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::Ok().json(PromptTemplateResponseDto::from(template)))
    }

    pub async fn list_prompt_templates(
        controller: web::Data<PromptTemplateController>,
        params: web::Query<PromptTemplateQueryParams>,
    ) -> ActixResult<HttpResponse> {
        let templates = if let Some(search) = &params.search {
            controller
                .prompt_template_repo
                .search(search)
                .await
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
        } else if let Some(category) = &params.category {
            controller
                .prompt_template_repo
                .find_by_category(category.as_ref())
                .await
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
        } else if params.system_only.unwrap_or(false) {
            controller
                .prompt_template_repo
                .find_system_prompts()
                .await?
        } else {
            controller.prompt_template_repo.find_all().await?
        };

        let response_dtos: Vec<PromptTemplateResponseDto> = templates
            .into_iter()
            .map(PromptTemplateResponseDto::from)
            .collect();

        Ok(HttpResponse::Ok().json(PromptTemplateListResponse {
            total: response_dtos.len(),
            templates: response_dtos,
        }))
    }

    pub async fn update_prompt_template(
        controller: web::Data<PromptTemplateController>,
        path: web::Path<Uuid>,
        request: web::Json<UpdatePromptTemplateRequest>,
    ) -> ActixResult<HttpResponse> {
        let dto = UpdatePromptTemplateDto {
            // Only map fields that exist in UpdatePromptTemplateRequest
            description: request.description.clone().flatten(),
            prompt_template: request.template_content.clone(),
            variables: request.variables.clone().flatten().map(|_| vec![]), // TODO: parse variables if needed
            tags: request.tags.clone().flatten(),
            status: request.status.clone().and_then(|s| match s.as_str() {
                "Active" => Some(PromptStatus::Active),
                "Inactive" => Some(PromptStatus::Inactive),
                "Draft" => Some(PromptStatus::Draft),
                "Archived" => Some(PromptStatus::Archived),
                _ => None,
            }),
            // All other fields set to None
            name: None,
            category: None,
            resource_type: None,
            workflow_type: None,
            version: None,
            is_active: None,
            is_system: None,
            prompt_type: None,
            template_content: None,
            is_system_prompt: None,
            parent_id: None,
            usage_count: None,
            last_used_at: None,
        };
        let template = controller
            .prompt_template_repo
            .update(&*path, dto)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

        Ok(HttpResponse::Ok().json(PromptTemplateResponseDto::from(template)))
    }

    pub async fn delete_prompt_template(
        controller: web::Data<PromptTemplateController>,
        path: web::Path<Uuid>,
    ) -> ActixResult<HttpResponse> {
        controller
            .prompt_template_repo
            .delete(&*path)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::NoContent().finish())
    }

    pub async fn render_prompt(
        controller: web::Data<PromptTemplateController>,
        path: web::Path<Uuid>,
        request: web::Json<RenderPromptRequest>,
    ) -> ActixResult<HttpResponse> {
        let template = controller
            .prompt_template_repo
            .find_by_id(&*path)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

        // Use prompt_template field from Model for rendering
        let mut rendered_content = template.prompt_template.clone();
        let variables_used = Self::extract_variables(&template.prompt_template);

        // Simple variable substitution (in a real implementation, use a proper template engine)
        let variables = request.variables.clone(); // Clone instead of move
        if let Some(variables) = variables {
            if let Value::Object(var_map) = variables {
                for (key, value) in var_map {
                    let placeholder = format!("{{{{{}}}}}", key);
                    let replacement = match value {
                        Value::String(s) => s,
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => serde_json::to_string(&value).unwrap_or_default(),
                    };
                    rendered_content = rendered_content.replace(&placeholder, &replacement);
                }
            }
        }

        Ok(HttpResponse::Ok().json(RenderPromptResponse {
            rendered_content,
            variables_used,
        }))
    }

    pub async fn get_categories(
        _controller: web::Data<PromptTemplateController>,
    ) -> ActixResult<HttpResponse> {
        let categories = vec![
            "DynamoDB".to_string(),
            "Database".to_string(),
            "Kubernetes".to_string(),
            "General".to_string(),
            "Security".to_string(),
            "Performance".to_string(),
            "Custom".to_string(),
        ];

        Ok(HttpResponse::Ok().json(categories))
    }

    pub async fn get_prompt_types(
        _controller: web::Data<PromptTemplateController>,
    ) -> ActixResult<HttpResponse> {
        let prompt_types = vec![
            "analysis".to_string(),
            "optimization".to_string(),
            "troubleshooting".to_string(),
            "security".to_string(),
            "custom".to_string(),
        ];

        Ok(HttpResponse::Ok().json(prompt_types))
    }

    fn extract_variables(template_content: &str) -> Vec<String> {
        // Simple regex-like extraction of {{variable}} patterns
        let mut variables = Vec::new();
        let mut chars = template_content.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '{' && chars.peek() == Some(&'{') {
                chars.next(); // consume second '{'
                let mut var_name = String::new();

                while let Some(ch) = chars.next() {
                    if ch == '}' && chars.peek() == Some(&'}') {
                        chars.next(); // consume second '}'
                        if !var_name.is_empty() && !variables.contains(&var_name) {
                            variables.push(var_name);
                        }
                        break;
                    } else {
                        var_name.push(ch);
                    }
                }
            }
        }

        variables
    }
}
