use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SeaORM entity definition for the AWS accounts table
/// This maps directly to the database schema
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "aws_accounts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub account_id: String,
    pub account_name: String,
    #[sea_orm(nullable)]
    pub profile: Option<String>,
    pub default_region: String,
    #[sea_orm(nullable, column_type = "Json")]
    pub regions: Option<serde_json::Value>,
    pub use_role: bool,
    #[sea_orm(nullable)]
    pub role_arn: Option<String>,
    #[sea_orm(nullable)]
    pub external_id: Option<String>,
    #[sea_orm(nullable)]
    pub access_key_id: Option<String>,
    #[sea_orm(nullable)]
    pub secret_access_key: Option<String>,
    // New auth strategy fields
    pub auth_type: String, // auto|profile|assume_role|web_identity|sso|instance_role|access_keys
    #[sea_orm(nullable)]
    pub source_profile: Option<String>,
    #[sea_orm(nullable)]
    pub sso_profile: Option<String>,
    #[sea_orm(nullable)]
    pub web_identity_token_file: Option<String>,
    #[sea_orm(nullable)]
    pub session_name: Option<String>,
    #[sea_orm(nullable)]
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// AWS Account domain model
/// This is used throughout the application for business logic
/// It maps to the database entity via conversions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainModel {
    pub id: Uuid,
    pub account_id: String,
    pub account_name: String,
    pub profile: Option<String>,
    pub default_region: String,
    pub regions: Option<Vec<String>>,
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    #[serde(skip_serializing)]
    pub secret_access_key: Option<String>,
    pub auth_type: String,
    pub source_profile: Option<String>,
    pub sso_profile: Option<String>,
    pub web_identity_token_file: Option<String>,
    pub session_name: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// DTO for creating a new AWS account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsAccountCreateDto {
    pub account_id: String,
    pub account_name: String,
    pub profile: Option<String>,
    pub default_region: String,
    pub regions: Option<Vec<String>>,
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    // Auth strategy fields (optional in create; default to 'auto')
    #[serde(default)]
    pub auth_type: Option<String>,
    pub source_profile: Option<String>,
    pub sso_profile: Option<String>,
    pub web_identity_token_file: Option<String>,
    pub session_name: Option<String>,
}

/// DTO for updating an existing AWS account
#[derive(Debug, Serialize, Deserialize)]
pub struct AwsAccountUpdateDto {
    pub account_id: String,
    pub account_name: String,
    pub profile: Option<String>,
    pub default_region: String,
    pub regions: Option<Vec<String>>,
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    // Auth strategy fields
    pub auth_type: Option<String>,
    pub source_profile: Option<String>,
    pub sso_profile: Option<String>,
    pub web_identity_token_file: Option<String>,
    pub session_name: Option<String>,
}

/// DTO for returning account information (without sensitive data)
#[derive(Debug, Serialize, Deserialize)]
pub struct AwsAccountDto {
    pub id: Uuid,
    pub account_id: String,
    pub account_name: String,
    pub profile: Option<String>,
    pub default_region: String,
    pub regions: Option<Vec<String>>,
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub has_access_key: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,
    #[serde(skip_serializing)]
    pub secret_access_key: Option<String>,
    pub auth_type: String,
    pub source_profile: Option<String>,
    pub sso_profile: Option<String>,
    pub web_identity_token_file: Option<String>,
    pub session_name: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response for sync operations
#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub success: bool,
    pub count: usize,
    pub message: String,
}

/// Convert SeaORM Entity to domain Model
/// This is used when retrieving data from the database
impl From<Model> for DomainModel {
    fn from(entity: Model) -> Self {
        Self {
            id: entity.id,
            account_id: entity.account_id,
            account_name: entity.account_name,
            profile: entity.profile,
            default_region: entity.default_region,
            regions: entity
                .regions
                .map(|r| serde_json::from_value(r).unwrap_or_default()),
            use_role: entity.use_role,
            role_arn: entity.role_arn,
            external_id: entity.external_id,
            access_key_id: entity.access_key_id,
            secret_access_key: entity.secret_access_key,
            auth_type: entity.auth_type,
            source_profile: entity.source_profile,
            sso_profile: entity.sso_profile,
            web_identity_token_file: entity.web_identity_token_file,
            session_name: entity.session_name,
            last_synced_at: entity.last_synced_at,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }
}

/// Convert domain Model to DTO for API responses
/// This hides sensitive information like access keys
impl From<DomainModel> for AwsAccountDto {
    fn from(model: DomainModel) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            account_name: model.account_name,
            profile: model.profile,
            default_region: model.default_region,
            regions: model.regions,
            use_role: model.use_role,
            role_arn: model.role_arn,
            external_id: model.external_id,
            has_access_key: model.access_key_id.is_some(),
            access_key_id: None, // Initially None, set only when needed for editing
            secret_access_key: None, // Initially None, set only when needed for editing
            auth_type: model.auth_type,
            source_profile: model.source_profile,
            sso_profile: model.sso_profile,
            web_identity_token_file: model.web_identity_token_file,
            session_name: model.session_name,
            last_synced_at: model.last_synced_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// Convert creation DTO to ActiveModel for inserting new records
/// Generates a new UUID and sets creation timestamps
impl From<AwsAccountCreateDto> for ActiveModel {
    fn from(dto: AwsAccountCreateDto) -> Self {
        let now = Utc::now();
        let auth_type = dto.auth_type.unwrap_or_else(|| "auto".to_string());
        Self {
            id: Set(Uuid::new_v4()),
            account_id: Set(dto.account_id),
            account_name: Set(dto.account_name),
            profile: Set(dto.profile),
            default_region: Set(dto.default_region),
            regions: Set(dto
                .regions
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))),
            use_role: Set(dto.use_role),
            role_arn: Set(dto.role_arn),
            external_id: Set(dto.external_id),
            access_key_id: Set(dto.access_key_id),
            secret_access_key: Set(dto.secret_access_key),
            auth_type: Set(auth_type),
            source_profile: Set(dto.source_profile),
            sso_profile: Set(dto.sso_profile),
            web_identity_token_file: Set(dto.web_identity_token_file),
            session_name: Set(dto.session_name),
            last_synced_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
    }
}

/// Convert update DTO to ActiveModel for updating existing records
/// Preserves existing secret key if not provided in the update
/// Only updates the fields that are specified, leaving others unchanged
impl From<(AwsAccountUpdateDto, Option<String>, Uuid)> for ActiveModel {
    fn from((dto, existing_secret_key, id): (AwsAccountUpdateDto, Option<String>, Uuid)) -> Self {
        let now = Utc::now();
        let secret_key = if let Some(key) = dto.secret_access_key {
            if key.is_empty() {
                existing_secret_key
            } else {
                Some(key)
            }
        } else {
            existing_secret_key
        };

        Self {
            id: Set(id),
            account_id: Set(dto.account_id),
            account_name: Set(dto.account_name),
            profile: Set(dto.profile),
            default_region: Set(dto.default_region),
            regions: Set(dto
                .regions
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))),
            use_role: Set(dto.use_role),
            role_arn: Set(dto.role_arn),
            external_id: Set(dto.external_id),
            access_key_id: Set(dto.access_key_id),
            secret_access_key: Set(secret_key),
            auth_type: Set(dto.auth_type.unwrap_or_else(|| "auto".to_string())),
            source_profile: Set(dto.source_profile),
            sso_profile: Set(dto.sso_profile),
            web_identity_token_file: Set(dto.web_identity_token_file),
            session_name: Set(dto.session_name),
            last_synced_at: sea_orm::ActiveValue::NotSet,
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: Set(now),
        }
    }
}

impl AwsAccountDto {
    pub fn new_with_profile(profile: &str, region: &str) -> Self {
        let now = Utc::now();
        AwsAccountDto {
            id: Uuid::new_v4(),
            account_id: "".to_string(), // Will be filled later
            account_name: profile.to_string(),
            profile: Some(profile.to_string()),
            default_region: region.to_string(),
            regions: None,
            use_role: false,
            role_arn: None,
            external_id: None,
            has_access_key: false,
            access_key_id: None,
            secret_access_key: None,
            auth_type: "auto".to_string(),
            source_profile: None,
            sso_profile: None,
            web_identity_token_file: None,
            session_name: None,
            last_synced_at: None,
            created_at: now,
            updated_at: now,
        }
    }
}
