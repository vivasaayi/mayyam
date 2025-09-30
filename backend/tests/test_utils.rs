use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::sync::Arc;
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
        let temp_dir =
            TempDir::new().map_err(|e| sea_orm::DbErr::Custom(format!("Temp dir error: {}", e)))?;
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
    TEST_DB
        .get_or_init(|| async { TestDb::new().await.expect("Failed to create test database") })
        .await
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
    // For integration tests, we need to use the crate name directly
    // Note: In test context, we can't import from the main crate directly
    // These factories are for use in unit tests within the main crate

    /// Create a fake AWS account ID for testing
    pub fn fake_aws_account_id() -> String {
        fake::faker::number::en::NumberWithFormat("############").fake()
    }

    /// Create a fake company name for testing
    pub fn fake_company_name() -> String {
        fake::faker::company::en::CompanyName().fake()
    }

    /// Create a fake word for testing
    pub fn fake_word() -> String {
        fake::faker::lorem::en::Word().fake()
    }
}

/// Common test assertions
pub mod assertions {
    /// Assert that a result contains the expected data
    pub fn assert_contains_data<T, F>(items: &[T], predicate: F)
    where
        F: Fn(&T) -> bool,
    {
        assert!(
            items.iter().any(predicate),
            "Expected item not found in collection"
        );
    }
}
