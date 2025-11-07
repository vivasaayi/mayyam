use std::sync::Arc;

use serde_json::Value;
use tracing::instrument;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::llm_provider::{
    LlmPromptFormat, LlmProviderResponseDto, LlmProviderStatus, LlmProviderType,
};
use crate::repositories::llm_provider::LlmProviderRepository;

/// Business logic for managing LLM providers.
///
/// Controllers should depend on this service instead of talking to the
/// repository directly so that validation, cross-entity coordination and
/// telemetry stay in one place.
pub struct LlmProviderService {
    repo: Arc<LlmProviderRepository>,
}

impl LlmProviderService {
    pub fn new(repo: Arc<LlmProviderRepository>) -> Self {
        Self { repo }
    }

    /// Create a new LLM provider after enforcing basic validation rules.
    #[instrument(skip(self, input))]
    pub async fn create_provider(
        &self,
        input: CreateLlmProviderInput,
    ) -> Result<LlmProviderResponseDto, AppError> {
        if self.repo.find_by_name(&input.name).await?.is_some() {
            return Err(AppError::Conflict(format!(
                "LLM provider '{}' already exists",
                input.name
            )));
        }

        let model = self
            .repo
            .create(
                input.name,
                input.provider_type,
                input.model_name,
                input.api_endpoint,
                input.api_key,
                input.model_config,
                input.prompt_format,
                input.description,
                input.enabled,
                input.is_default,
            )
            .await?;

        Ok(LlmProviderResponseDto::from(model))
    }

    /// Fetch a provider by id, returning a 404 style error if not present.
    #[instrument(skip(self))]
    pub async fn get_provider(&self, id: Uuid) -> Result<LlmProviderResponseDto, AppError> {
        let provider = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("LLM provider {} not found", id)))?;

        Ok(LlmProviderResponseDto::from(provider))
    }

    /// List providers filtered by the supplied criteria.
    #[instrument(skip(self, filter))]
    pub async fn list_providers(
        &self,
        filter: ListLlmProvidersFilter,
    ) -> Result<Vec<LlmProviderResponseDto>, AppError> {
        let mut providers =
            if filter.active_only || matches!(filter.status, Some(LlmProviderStatus::Active)) {
                self.repo.find_active().await?
            } else if let Some(provider_type) = filter.provider_type.clone() {
                self.repo.find_by_provider_type(provider_type).await?
            } else {
                self.repo.find_all().await?
            };

        if let Some(status) = filter.status {
            providers = providers
                .into_iter()
                .filter(|p| match status {
                    LlmProviderStatus::Active => p.enabled,
                    LlmProviderStatus::Inactive => !p.enabled,
                    _ => true,
                })
                .collect();
        }

        Ok(providers
            .into_iter()
            .map(LlmProviderResponseDto::from)
            .collect())
    }

    /// Update a provider while ensuring uniqueness constraints.
    #[instrument(skip(self, input))]
    pub async fn update_provider(
        &self,
        id: Uuid,
        input: UpdateLlmProviderInput,
    ) -> Result<LlmProviderResponseDto, AppError> {
        if let Some(ref name) = input.name {
            if let Some(existing) = self.repo.find_by_name(name).await? {
                if existing.id != id {
                    return Err(AppError::Conflict(format!(
                        "Another LLM provider named '{}' already exists",
                        name
                    )));
                }
            }
        }

        let model = self
            .repo
            .update(
                id,
                input.name,
                input.model_name,
                input.api_endpoint,
                input.api_key,
                input.model_config,
                input.prompt_format,
                input.description,
                input.status,
                input.enabled,
                input.is_default,
            )
            .await?;

        Ok(LlmProviderResponseDto::from(model))
    }

    /// Delete a provider by id.
    #[instrument(skip(self))]
    pub async fn delete_provider(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.delete(id).await
    }

    /// Execute a lightweight connectivity check for a provider.
    #[instrument(skip(self))]
    pub async fn test_provider(&self, id: Uuid) -> Result<bool, AppError> {
        self.repo.test_connection(id).await
    }

    /// All supported provider type labels.
    pub fn list_provider_types() -> Vec<String> {
        vec![
            "OpenAI".to_string(),
            "Ollama".to_string(),
            "Anthropic".to_string(),
            "Local".to_string(),
            "Gemini".to_string(),
            "DeepSeek".to_string(),
            "Custom".to_string(),
        ]
    }

    /// Supported prompt format labels.
    pub fn list_prompt_formats() -> Vec<String> {
        vec![
            "OpenAI".to_string(),
            "Anthropic".to_string(),
            "Custom".to_string(),
        ]
    }
}

#[derive(Clone, Debug)]
pub struct CreateLlmProviderInput {
    pub name: String,
    pub provider_type: LlmProviderType,
    pub model_name: String,
    pub api_endpoint: Option<String>,
    pub api_key: Option<String>,
    pub model_config: Option<Value>,
    pub prompt_format: LlmPromptFormat,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct ListLlmProvidersFilter {
    pub provider_type: Option<LlmProviderType>,
    pub status: Option<LlmProviderStatus>,
    pub active_only: bool,
}

#[derive(Clone, Debug)]
pub struct UpdateLlmProviderInput {
    pub name: Option<String>,
    pub model_name: Option<String>,
    pub api_endpoint: Option<Option<String>>,
    pub api_key: Option<Option<String>>,
    pub model_config: Option<Option<Value>>,
    pub prompt_format: Option<LlmPromptFormat>,
    pub description: Option<Option<String>>,
    pub status: Option<LlmProviderStatus>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

impl UpdateLlmProviderInput {
    pub fn new() -> Self {
        Self {
            name: None,
            model_name: None,
            api_endpoint: None,
            api_key: None,
            model_config: None,
            prompt_format: None,
            description: None,
            status: None,
            enabled: None,
            is_default: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::errors::AppError;
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
    use serde_json::json;

    async fn setup_service() -> LlmProviderService {
        let connection = Arc::new(
            Database::connect("sqlite::memory:?cache=shared")
                .await
                .expect("sqlite memory db"),
        );

        connection
            .as_ref()
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                r#"
                CREATE TABLE IF NOT EXISTS llm_providers (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    provider_type TEXT NOT NULL,
                    base_url TEXT,
                    api_key TEXT,
                    model_name TEXT NOT NULL,
                    model_config TEXT NOT NULL DEFAULT '{}',
                    prompt_format TEXT NOT NULL,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    is_default INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                "#,
            ))
            .await
            .expect("create table");

        let repo = Arc::new(LlmProviderRepository::new(connection, Config::default()));
        LlmProviderService::new(repo)
    }

    fn build_create_input(name: &str) -> CreateLlmProviderInput {
        CreateLlmProviderInput {
            name: name.to_string(),
            provider_type: LlmProviderType::OpenAI,
            model_name: "gpt-4o".to_string(),
            api_endpoint: Some("https://api.openai.com".to_string()),
            api_key: Some("test-key".to_string()),
            model_config: Some(json!({"temperature": 0.7})),
            prompt_format: LlmPromptFormat::OpenAI,
            description: None,
            enabled: Some(true),
            is_default: Some(false),
        }
    }

    #[tokio::test]
    async fn create_and_fetch_provider_round_trip() {
        let service = setup_service().await;
        let created = service
            .create_provider(build_create_input("Primary LLM"))
            .await
            .expect("create provider");

        let fetched = service
            .get_provider(created.id)
            .await
            .expect("fetch provider");

        assert_eq!(created.id, fetched.id);
        assert_eq!(fetched.name, "Primary LLM");
        assert!(fetched.has_api_key);
    }

    #[tokio::test]
    async fn duplicate_provider_name_is_rejected() {
        let service = setup_service().await;

        service
            .create_provider(build_create_input("Shared LLM"))
            .await
            .expect("first provider");

        let err = service
            .create_provider(build_create_input("Shared LLM"))
            .await
            .expect_err("duplicate should fail");

        assert!(matches!(err, AppError::Conflict(_)));
    }

    #[tokio::test]
    async fn list_filters_active_providers() {
        let service = setup_service().await;

        service
            .create_provider(build_create_input("Active"))
            .await
            .expect("active provider");

        let mut disabled_input = build_create_input("Offline");
        disabled_input.enabled = Some(false);
        service
            .create_provider(disabled_input)
            .await
            .expect("disabled provider");

        let active_only = service
            .list_providers(ListLlmProvidersFilter {
                provider_type: None,
                status: None,
                active_only: true,
            })
            .await
            .expect("list active");

        assert_eq!(active_only.len(), 1);
        assert_eq!(active_only[0].name, "Active");
    }

    #[tokio::test]
    async fn update_provider_reuses_validation() {
        let service = setup_service().await;

        let created = service
            .create_provider(build_create_input("Baseline"))
            .await
            .expect("create baseline");

        let update_result = service
            .update_provider(
                created.id,
                UpdateLlmProviderInput {
                    name: Some("Renamed".to_string()),
                    model_name: Some("gpt-4o-mini".to_string()),
                    api_endpoint: None,
                    api_key: None,
                    model_config: Some(Some(json!({"temperature": 0.2}))),
                    prompt_format: Some(LlmPromptFormat::OpenAI),
                    description: None,
                    status: None,
                    enabled: Some(true),
                    is_default: Some(false),
                },
            )
            .await
            .expect("update provider");

        assert_eq!(update_result.name, "Renamed");

        // Attempt to rename to something that already exists and expect a conflict
        service
            .create_provider(build_create_input("Existing"))
            .await
            .expect("second provider");

        let conflict = service
            .update_provider(
                update_result.id,
                UpdateLlmProviderInput {
                    name: Some("Existing".to_string()),
                    ..UpdateLlmProviderInput::new()
                },
            )
            .await
            .expect_err("conflict expected");

        assert!(matches!(conflict, AppError::Conflict(_)));
    }
}
