use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Prompt template entity for managing external prompts
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "prompt_templates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub category: String, // e.g., "aws_analysis", "database_optimization", "troubleshooting"
    pub resource_type: Option<String>, // e.g., "DynamoDB", "RDS", "Kubernetes" - null for generic prompts
    pub workflow_type: Option<String>, // e.g., "Performance", "Cost", "5-Why" - null for generic prompts
    pub prompt_template: String,       // The actual prompt template with variables
    pub variables: Json,               // Available variables and their descriptions
    pub version: String,
    pub is_active: bool,
    pub is_system: bool, // System prompts vs user-created prompts
    pub description: Option<String>,
    pub tags: Json, // Array of tags for categorization
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>, // User who created this prompt
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// Domain model for prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplateDomain {
    pub id: Uuid,
    pub name: String,
    pub category: String,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: String,
    pub variables: Vec<PromptVariable>,
    pub version: String,
    pub is_active: bool,
    pub is_system: bool,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

/// Variable definition in prompt templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVariable {
    pub name: String,
    pub description: String,
    pub variable_type: PromptVariableType,
    pub required: bool,
    pub default_value: Option<String>,
    pub example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PromptVariableType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    MetricData,
    ResourceConfig,
}

/// DTO for creating new prompt template
#[derive(Debug, Deserialize)]
pub struct PromptTemplateCreateDto {
    pub name: String,
    pub category: String,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: String,
    pub variables: Vec<PromptVariable>,
    pub version: Option<String>,
    pub is_active: Option<bool>,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

/// DTO for updating prompt template
#[derive(Debug, Deserialize)]
pub struct PromptTemplateUpdateDto {
    pub name: Option<String>,
    pub category: Option<String>,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: Option<String>,
    pub variables: Option<Vec<PromptVariable>>,
    pub version: Option<String>,
    pub is_active: Option<bool>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// DTO for API responses
#[derive(Debug, Serialize)]
pub struct PromptTemplateDto {
    pub id: Uuid,
    pub name: String,
    pub category: String,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: String,
    pub variables: Vec<PromptVariable>,
    pub version: String,
    pub is_active: bool,
    pub is_system: bool,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

/// Prompt execution request
#[derive(Debug, Deserialize)]
pub struct PromptExecutionRequest {
    pub template_id: Uuid,
    pub variables: serde_json::Value,
    pub llm_provider_id: Option<Uuid>,
}

/// Prompt execution response
#[derive(Debug, Serialize)]
pub struct PromptExecutionResponse {
    pub template_id: Uuid,
    pub template_name: String,
    pub llm_provider_name: String,
    pub rendered_prompt: String,
    pub response: String,
    pub execution_time_ms: u64,
    pub token_usage: Option<TokenUsage>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// Add missing enum types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PromptCategory {
    AwsAnalysis,
    DatabaseOptimization,
    Troubleshooting,
    Performance,
    Cost,
    Security,
    General,
}

impl From<String> for PromptCategory {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "aws_analysis" | "awsanalysis" => Self::AwsAnalysis,
            "database_optimization" | "databaseoptimization" => Self::DatabaseOptimization,
            "troubleshooting" => Self::Troubleshooting,
            "performance" => Self::Performance,
            "cost" => Self::Cost,
            "security" => Self::Security,
            _ => Self::General,
        }
    }
}

impl From<PromptCategory> for String {
    fn from(category: PromptCategory) -> Self {
        match category {
            PromptCategory::AwsAnalysis => "aws_analysis".to_string(),
            PromptCategory::DatabaseOptimization => "database_optimization".to_string(),
            PromptCategory::Troubleshooting => "troubleshooting".to_string(),
            PromptCategory::Performance => "performance".to_string(),
            PromptCategory::Cost => "cost".to_string(),
            PromptCategory::Security => "security".to_string(),
            PromptCategory::General => "general".to_string(),
        }
    }
}

impl fmt::Display for PromptCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: String = self.clone().into();
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PromptType {
    System,
    User,
    Analysis,
    Workflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PromptStatus {
    Active,
    Inactive,
    Draft,
    Archived,
}

// Add missing request/response DTOs
#[derive(Debug, Deserialize)]
pub struct CreatePromptTemplateRequest {
    pub name: String,
    pub category: PromptCategory,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: String,
    pub variables: Vec<PromptVariable>,
    pub version: Option<String>,
    pub is_active: Option<bool>,
    pub is_system: Option<bool>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub prompt_type: Option<PromptType>,
    pub template_content: Option<String>,
    pub is_system_prompt: Option<bool>,
    pub parent_id: Option<Uuid>,
    pub status: Option<PromptStatus>,
    pub usage_count: Option<i64>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePromptTemplateRequest {
    pub name: Option<String>,
    pub category: Option<PromptCategory>,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: Option<String>,
    pub variables: Option<Vec<PromptVariable>>,
    pub version: Option<String>,
    pub is_active: Option<bool>,
    pub is_system: Option<bool>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub prompt_type: Option<PromptType>,
    pub template_content: Option<String>,
    pub is_system_prompt: Option<bool>,
    pub parent_id: Option<Uuid>,
    pub status: Option<PromptStatus>,
    pub usage_count: Option<i64>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct PromptTemplateQueryParams {
    pub name: Option<String>,
    pub category: Option<PromptCategory>,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub is_active: Option<bool>,
    pub is_system: Option<bool>,
    pub status: Option<PromptStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePromptTemplateDto {
    pub name: String,
    pub category: PromptCategory,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: String,
    pub variables: Vec<PromptVariable>,
    pub version: Option<String>,
    pub is_active: Option<bool>,
    pub is_system: Option<bool>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub prompt_type: Option<PromptType>,
    pub template_content: Option<String>,
    pub is_system_prompt: Option<bool>,
    pub parent_id: Option<Uuid>,
    pub status: Option<PromptStatus>,
    pub usage_count: Option<i64>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePromptTemplateDto {
    pub name: Option<String>,
    pub category: Option<PromptCategory>,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: Option<String>,
    pub variables: Option<Vec<PromptVariable>>,
    pub version: Option<String>,
    pub is_active: Option<bool>,
    pub is_system: Option<bool>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub prompt_type: Option<PromptType>,
    pub template_content: Option<String>,
    pub is_system_prompt: Option<bool>,
    pub parent_id: Option<Uuid>,
    pub status: Option<PromptStatus>,
    pub usage_count: Option<i64>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct PromptTemplateResponseDto {
    pub id: Uuid,
    pub name: String,
    pub category: PromptCategory,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub prompt_template: String,
    pub variables: Vec<PromptVariable>,
    pub version: String,
    pub is_active: bool,
    pub is_system: bool,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub prompt_type: Option<PromptType>,
    pub template_content: Option<String>,
    pub is_system_prompt: Option<bool>,
    pub parent_id: Option<Uuid>,
    pub status: PromptStatus,
    pub usage_count: Option<i64>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

// Conversions
impl From<Model> for PromptTemplateDomain {
    fn from(entity: Model) -> Self {
        let variables: Vec<PromptVariable> =
            serde_json::from_value(entity.variables).unwrap_or_default();

        let tags: Vec<String> = serde_json::from_value(entity.tags).unwrap_or_default();

        Self {
            id: entity.id,
            name: entity.name,
            category: entity.category,
            resource_type: entity.resource_type,
            workflow_type: entity.workflow_type,
            prompt_template: entity.prompt_template,
            variables,
            version: entity.version,
            is_active: entity.is_active,
            is_system: entity.is_system,
            description: entity.description,
            tags,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
            created_by: entity.created_by,
        }
    }
}

impl From<PromptTemplateDomain> for PromptTemplateDto {
    fn from(domain: PromptTemplateDomain) -> Self {
        Self {
            id: domain.id,
            name: domain.name,
            category: domain.category,
            resource_type: domain.resource_type,
            workflow_type: domain.workflow_type,
            prompt_template: domain.prompt_template,
            variables: domain.variables,
            version: domain.version,
            is_active: domain.is_active,
            is_system: domain.is_system,
            description: domain.description,
            tags: domain.tags,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
            created_by: domain.created_by,
        }
    }
}

impl From<PromptTemplateCreateDto> for ActiveModel {
    fn from(dto: PromptTemplateCreateDto) -> Self {
        let now = Utc::now();
        let variables_json =
            serde_json::to_value(dto.variables).unwrap_or(serde_json::Value::Array(vec![]));
        let tags_json = serde_json::to_value(dto.tags).unwrap_or(serde_json::Value::Array(vec![]));

        Self {
            id: Set(Uuid::new_v4()),
            name: Set(dto.name),
            category: Set(String::from(dto.category)),
            resource_type: Set(dto.resource_type),
            workflow_type: Set(dto.workflow_type),
            prompt_template: Set(dto.prompt_template),
            variables: Set(variables_json),
            version: Set(dto.version.unwrap_or("1.0".to_string())),
            is_active: Set(dto.is_active.unwrap_or(true)),
            is_system: Set(false), // User-created prompts are not system prompts
            description: Set(dto.description),
            tags: Set(tags_json),
            created_at: Set(now),
            updated_at: Set(now),
            created_by: Set(None), // Will be set by the service based on authentication
        }
    }
}

// Implement From<Model> for PromptTemplateResponseDto
impl From<Model> for PromptTemplateResponseDto {
    fn from(entity: Model) -> Self {
        let variables: Vec<PromptVariable> =
            serde_json::from_value(entity.variables.clone()).unwrap_or_default();

        let tags: Vec<String> = serde_json::from_value(entity.tags.clone()).unwrap_or_default();

        // Parse category from string
        let category = PromptCategory::from(entity.category);

        Self {
            id: entity.id,
            name: entity.name,
            category,
            resource_type: entity.resource_type,
            workflow_type: entity.workflow_type,
            prompt_template: entity.prompt_template,
            variables,
            version: entity.version,
            is_active: entity.is_active,
            is_system: entity.is_system,
            description: entity.description,
            tags,
            prompt_type: None,                        // Not in base model
            template_content: None,                   // Not in base model
            is_system_prompt: Some(entity.is_system), // Map is_system to is_system_prompt
            parent_id: None,                          // Not in base model
            status: PromptStatus::Active,             // Default status
            usage_count: None,                        // Not in base model
            last_used_at: None,                       // Not in base model
            created_at: entity.created_at,
            updated_at: entity.updated_at,
            created_by: entity.created_by,
        }
    }
}
