use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::Set;
use sea_orm::{entity::prelude::*, ActiveValue};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub username: String,
    pub email: String,
    #[sea_orm(column_type = "Text")]
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub active: bool, // Matches database schema 'active' column
    #[sea_orm(column_type = "Text")]
    pub roles: String, // Matches database schema 'roles' column
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub updated_at: DateTime<Utc>,
    #[sea_orm(column_type = "TimestampWithTimeZone", nullable)]
    pub last_login: Option<DateTime<Utc>>,

    // Adding permissions as transient field derived from roles
    #[sea_orm(ignore)]
    #[serde(skip_deserializing)]
    pub permissions: Vec<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {
    // Before save hook - set timestamps
    //fn before_save(mut self) -> Result<Self, DbErr> {
    // if insert {
    //     self.id = Set(Uuid::new_v4());
    //     self.created_at = Set(Utc::now().naive_utc());
    //     self.updated_at = Set(Utc::now().naive_utc());
    //     self.is_active = Set(true);

    //     // Check if permissions field is unchanged and set default if needed
    //     match &self.permissions {
    //         ActiveValue::Unchanged(_) | ActiveValue::NotSet => {
    //             self.permissions = Set(vec!["user".to_string()]);
    //         }
    //         _ => {}
    //     }
    // } else {
    //     self.updated_at = Set(Utc::now().naive_utc());
    // }

    // Ok(self)
    //}
}

// Dto for user creation
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserDto {
    pub username: String,
    pub email: String,
    pub password: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_admin: Option<bool>,
    pub permissions: Option<Vec<String>>,
}

// Dto for user login
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginUserDto {
    pub username: String,
    pub password: String,
}

// Dto for user update
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserDto {
    pub email: Option<String>,
    pub password: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_active: Option<bool>,
    pub is_admin: Option<bool>,
    pub permissions: Option<Vec<String>>,
}

// Auth token response
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthTokenResponse {
    pub token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

// User response (safe to send to frontend)
#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_active: bool,
    pub is_admin: bool,
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
}

impl From<Model> for UserResponse {
    fn from(user: Model) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            first_name: user.first_name,
            last_name: user.last_name,
            is_active: user.active,
            is_admin: false,
            permissions: user.permissions,
            created_at: user.created_at,
            last_login: user.last_login,
        }
    }
}

// JWT claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid, // user id
    pub username: String,
    pub permissions: Vec<String>,
    pub is_admin: bool,
    pub exp: i64, // expiration time (as UTC timestamp)
    pub iat: i64, // issued at (as UTC timestamp)
}
