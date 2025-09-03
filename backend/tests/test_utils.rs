use std::sync::Arc;
use sea_orm::{Database, DatabaseConnection, ConnectOptions};
use tempfile::TempDir;
use tokio::sync::OnceCell;

/// Test database setup utilities
pub struct TestDb {
    pub connection: DatabaseConnection,
    _temp_dir: TempDir, // Keep temp dir alive
}

impl TestDb {
    /// Create a new in-memory SQLite database for testing
    pub async fn new() -> Result<Self, sea_orm::DbErr> {
        let temp_dir = TempDir::new().map_err(|e| sea_orm::DbErr::Custom(format!("Temp dir error: {}", e)))?;
        let db_path = temp_dir.path().join("test.db");

        let mut opt = ConnectOptions::new(format!("sqlite://{}?mode=rwc", db_path.display()));
        opt.max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);

        let connection = Database::connect(opt).await?;

        Ok(Self {
            connection,
            _temp_dir: temp_dir,
        })
    }

    /// Get a reference to the database connection
    pub fn conn(&self) -> &DatabaseConnection {
        &self.connection
    }
}

/// Global test database instance for integration tests
static TEST_DB: OnceCell<TestDb> = OnceCell::const_new();

/// Get or create the global test database instance
async fn get_test_db() -> &'static TestDb {
    TEST_DB.get_or_init(|| async {
        TestDb::new().await.expect("Failed to create test database")
    }).await
}

/// Setup test database for integration tests
pub async fn setup_test_database() -> DatabaseConnection {
    let test_db = get_test_db().await;
    test_db.conn().clone()
}

/// Cleanup test database (no-op for SQLite in-memory)
pub async fn cleanup_test_database() {
    // SQLite in-memory database is automatically cleaned up
}

/// Test configuration for unit tests
pub struct TestConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub server_port: u16,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            database_url: "sqlite::memory:".to_string(),
            jwt_secret: "test_jwt_secret_key_for_testing_only".to_string(),
            server_port: 8080,
        }
    }
}

/// Mock AWS service for testing
#[cfg(test)]
use mockall::mock;

#[cfg(test)]
mock! {
    pub AwsService {}
    impl Clone for AwsService {
        fn clone(&self) -> Self;
    }
}

/// Test data factories
pub mod factories {
    use fake::Fake;
    use uuid::Uuid;
    // For integration tests, we need to use the crate name directly
    extern crate mayyam;
    use mayyam::models::aws_account::{AwsAccountCreateDto, DomainModel};

    /// Create a fake AWS account for testing
    pub fn fake_aws_account() -> AwsAccountCreateDto {
        AwsAccountCreateDto {
            account_id: fake::faker::number::en::NumberWithFormat("############").fake(),
            account_name: fake::faker::company::en::CompanyName().fake(),
            profile: Some(fake::faker::lorem::en::Word().fake()),
            default_region: "us-east-1".to_string(),
            use_role: false,
            role_arn: None,
            external_id: None,
            access_key_id: Some(fake::faker::lorem::en::Word().fake()),
            secret_access_key: Some(fake::faker::lorem::en::Word().fake()),
        }
    }

    /// Create a fake domain model for testing
    pub fn fake_domain_model() -> DomainModel {
        DomainModel {
            id: Uuid::new_v4(),
            account_id: fake::faker::number::en::NumberWithFormat("############").fake(),
            account_name: fake::faker::company::en::CompanyName().fake(),
            profile: Some(fake::faker::lorem::en::Word().fake()),
            default_region: "us-east-1".to_string(),
            use_role: false,
            role_arn: None,
            external_id: None,
            access_key_id: Some(fake::faker::lorem::en::Word().fake()),
            secret_access_key: Some(fake::faker::lorem::en::Word().fake()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

/// Common test assertions
pub mod assertions {
    use crate::errors::AppError;

    /// Assert that a result is an error of a specific type
    pub fn assert_error_type<T>(result: &Result<T, AppError>, expected_error: &str) {
        match result {
            Err(AppError::Validation(msg)) if msg.contains(expected_error) => {}
            Err(AppError::NotFound(msg)) if msg.contains(expected_error) => {}
            Err(AppError::Database(_)) if expected_error == "database" => {}
            Err(AppError::Authentication(msg)) if msg.contains(expected_error) => {}
            _ => panic!("Expected error containing '{}', got {:?}", expected_error, result),
        }
    }

    /// Assert that a result contains the expected data
    pub fn assert_contains_data<T, F>(items: &[T], predicate: F)
    where
        F: Fn(&T) -> bool,
    {
        assert!(items.iter().any(predicate), "Expected item not found in collection");
    }
}
