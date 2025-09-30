use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use tempfile::TempDir;
use tokio::sync::OnceCell;

/// Test database setup utilities
pub struct TestDb {
    pub connection: DatabaseConnection,
    _temp_dir: TempDir,
}

impl TestDb {
    pub async fn new() -> Result<Self, sea_orm::DbErr> {
        let temp_dir = TempDir::new()
            .map_err(|e| sea_orm::DbErr::Custom(format!("Temp dir error: {e}")))?;
        let db_path = temp_dir.path().join("test.db");

        let mut options = ConnectOptions::new(format!("sqlite://{}?mode=rwc", db_path.display()));
        options.max_connections(1).min_connections(1).sqlx_logging(false);

        let connection = Database::connect(options).await?;

        Ok(Self {
            connection,
            _temp_dir: temp_dir,
        })
    }

    pub fn conn(&self) -> &DatabaseConnection {
        &self.connection
    }
}

static TEST_DB: OnceCell<TestDb> = OnceCell::const_new();

pub async fn get_test_db() -> &'static TestDb {
    TEST_DB
        .get_or_init(|| async { TestDb::new().await.expect("Failed to create test database") })
        .await
}

pub async fn setup_test_database() -> DatabaseConnection {
    let test_db = get_test_db().await;
    test_db.conn().clone()
}

pub async fn cleanup_test_database() {
    // SQLite in-memory database is automatically cleaned up
}

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

#[cfg(test)]
use mockall::mock;

#[cfg(test)]
mock! {
    pub AwsService {}
    impl Clone for AwsService {
        fn clone(&self) -> Self;
    }
}

pub mod factories {
    use fake::Fake;
    use mayyam::models::aws_account::{AwsAccountCreateDto, DomainModel};
    use uuid::Uuid;

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
            last_synced_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

pub mod assertions {
    pub fn assert_contains_data<T, F>(items: &[T], predicate: F)
    where
        F: Fn(&T) -> bool,
    {
        assert!(items.iter().any(predicate), "Expected item not found in collection");
    }
}
