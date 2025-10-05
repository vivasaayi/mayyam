use crate::config::Config;
use crate::errors::AppError;
use crate::models::database::Model as DatabaseConnectionModel;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm::{Database, DatabaseConnection, DbErr};
use tracing::{error, info};

/// Connect to the database using configuration settings
pub async fn connect(config: &Config) -> Result<DatabaseConnection, DbErr> {
    // Use the first PostgreSQL configuration for the application database
    if let Some(pg_config) = config.database.postgres.first() {
        let database_url = format!(
            "postgres://{}:{}@{}:{}/{}",
            pg_config.username,
            pg_config.password,
            pg_config.host,
            pg_config.port,
            pg_config.database
        );

        info!("Connecting to PostgreSQL database: {}", pg_config.name);

        let conn = Database::connect(database_url).await?;
        info!("Database connection established successfully");

        return Ok(conn);
    } else {
        error!("No PostgreSQL database configuration found");
        return Err(DbErr::Custom(
            "No PostgreSQL database configuration found".to_string(),
        ));
    }
}

pub async fn connect_to_specific_postgres(
    config: &crate::config::PostgresConfig,
) -> Result<DatabaseConnection, DbErr> {
    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        config.username, config.password, config.host, config.port, config.database
    );

    info!("Connecting to PostgreSQL database: {}", config.name);
    let conn = Database::connect(database_url).await?;

    Ok(conn)
}

pub async fn connect_to_specific_mysql(
    config: &crate::config::MySQLConfig,
) -> Result<DatabaseConnection, DbErr> {
    let database_url = format!(
        "mysql://{}:{}@{}:{}/{}",
        config.username, config.password, config.host, config.port, config.database
    );

    info!("Connecting to MySQL database: {}", config.name);
    info!(database_url);
    info!("OOOOOH : {}", config.name);
    let conn = Database::connect(database_url).await?;

    Ok(conn)
}

/// Decrypt a password using the application's encryption key
pub fn decrypt_password(encrypted: &str, config: &Config) -> Result<String, AppError> {
    let encryption_key = config.security.encryption_key.as_bytes();
    if encryption_key.len() != 32 {
        return Err(AppError::Config(
            "Invalid encryption key length".to_string(),
        ));
    }

    // Decode base64
    let data = BASE64
        .decode(encrypted)
        .map_err(|e| AppError::Internal(format!("Base64 decoding error: {}", e)))?;

    if data.len() < 12 {
        // nonce is 12 bytes
        return Err(AppError::Internal("Invalid encrypted data".to_string()));
    }

    // Split nonce and ciphertext
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Create cipher
    let cipher = Aes256Gcm::new(encryption_key.into());

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| AppError::Internal(format!("Decryption error: {}", e)))?;

    String::from_utf8(plaintext)
        .map_err(|e| AppError::Internal(format!("UTF-8 decoding error: {}", e)))
}

/// Connect to a database based on a dynamic database connection model
pub async fn connect_to_dynamic_database(
    conn_model: &DatabaseConnectionModel,
    config: &Config,
) -> Result<DatabaseConnection, AppError> {
    // Decrypt password using the standalone decryption function
    let password = if let Some(encrypted_password) = &conn_model.password_encrypted {
        decrypt_password(encrypted_password, config)?
    } else {
        return Err(AppError::BadRequest(
            "No password found for database connection".to_string(),
        ));
    };

    let database_url = match conn_model.connection_type.as_str() {
        "mysql" => {
            let db_name = conn_model.database_name.as_deref().unwrap_or("mysql");
            format!(
                "mysql://{}:{}@{}:{}/{}",
                conn_model.username.as_deref().unwrap_or("root"),
                password,
                conn_model.host,
                conn_model.port,
                db_name
            )
        }
        "postgres" => {
            let db_name = conn_model.database_name.as_deref().unwrap_or("postgres");
            format!(
                "postgres://{}:{}@{}:{}/{}",
                conn_model.username.as_deref().unwrap_or("postgres"),
                password,
                conn_model.host,
                conn_model.port,
                db_name
            )
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Unsupported database type for dynamic connection: {}",
                conn_model.connection_type
            )));
        }
    };

    info!(
        "Connecting to {} database: {}",
        conn_model.connection_type, conn_model.name
    );

    let conn = Database::connect(database_url)
        .await
        .map_err(|e| AppError::Database(e))?;

    Ok(conn)
}

/// Test a database connection
pub async fn test_database_connection(
    conn_model: &DatabaseConnectionModel,
    config: &Config,
) -> Result<(), AppError> {
    let conn = connect_to_dynamic_database(conn_model, config).await?;

    // Try a simple query to test the connection
    match conn_model.connection_type.as_str() {
        "mysql" => {
            use sea_orm::{ConnectionTrait, DbBackend, Statement};
            let _result = conn
                .execute(Statement::from_string(
                    DbBackend::MySql,
                    "SELECT 1".to_string(),
                ))
                .await
                .map_err(AppError::Database)?;
        }
        "postgres" => {
            use sea_orm::{ConnectionTrait, DbBackend, Statement};
            let _result = conn
                .execute(Statement::from_string(
                    DbBackend::Postgres,
                    "SELECT 1".to_string(),
                ))
                .await
                .map_err(AppError::Database)?;
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Connection testing not supported for database type: {}",
                conn_model.connection_type
            )));
        }
    }

    Ok(())
}

/// Ensure the llm_provider_models table exists to support multiple models per provider.
/// This is a targeted, idempotent setup used at startup to avoid 500s when saving models
/// in environments where migrations haven't been applied.
pub async fn ensure_llm_provider_models_table(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Create table if it doesn't exist
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS llm_provider_models (
            id UUID PRIMARY KEY,
            provider_id UUID NOT NULL REFERENCES llm_providers(id) ON DELETE CASCADE,
            model_name VARCHAR(255) NOT NULL,
            model_config JSONB NOT NULL DEFAULT '{}'::jsonb,
            enabled BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#;

    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_table_sql.to_string(),
    ))
    .await?;

    // Create indexes if they don't exist
    let create_idx_provider = r#"CREATE INDEX IF NOT EXISTS idx_llm_provider_models_provider ON llm_provider_models(provider_id)"#;
    let create_idx_enabled = r#"CREATE INDEX IF NOT EXISTS idx_llm_provider_models_enabled ON llm_provider_models(enabled)"#;

    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_idx_provider.to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_idx_enabled.to_string(),
    ))
    .await?;

    Ok(())
}

/// Ensure the sync_runs table exists to support tracking sync sessions.
/// Idempotent creation to avoid runtime failures when migrations haven't been applied.
pub async fn ensure_sync_runs_table(db: &DatabaseConnection) -> Result<(), DbErr> {
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS sync_runs (
            id UUID PRIMARY KEY,
            name TEXT NOT NULL,
            aws_account_id UUID NULL,
            account_id TEXT NULL,
            profile TEXT NULL,
            region TEXT NULL,
            status TEXT NOT NULL DEFAULT 'created',
            total_resources INTEGER NOT NULL DEFAULT 0,
            success_count INTEGER NOT NULL DEFAULT 0,
            failure_count INTEGER NOT NULL DEFAULT 0,
            error_summary TEXT NULL,
            metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
            started_at TIMESTAMPTZ NULL,
            completed_at TIMESTAMPTZ NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#;

    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_table_sql.to_string(),
    ))
    .await?;

    let create_idx_created =
        r#"CREATE INDEX IF NOT EXISTS idx_sync_runs_created_at ON sync_runs (created_at DESC)"#;
    let create_idx_status =
        r#"CREATE INDEX IF NOT EXISTS idx_sync_runs_status ON sync_runs (status)"#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_idx_created.to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_idx_status.to_string(),
    ))
    .await?;

    // Trigger to auto-update updated_at
    let create_fn = r#"
        CREATE OR REPLACE FUNCTION set_updated_at()
        RETURNS TRIGGER AS $$
        BEGIN
          NEW.updated_at = NOW();
          RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
    "#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_fn.to_string(),
    ))
    .await?;

    let drop_trigger = r#"DROP TRIGGER IF EXISTS trg_sync_runs_updated_at ON sync_runs"#;
    let create_trigger = r#"
        CREATE TRIGGER trg_sync_runs_updated_at
        BEFORE UPDATE ON sync_runs
        FOR EACH ROW
        EXECUTE PROCEDURE set_updated_at();
    "#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        drop_trigger.to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_trigger.to_string(),
    ))
    .await?;

    Ok(())
}

/// Ensure aws_resources table has sync_id and correct indexes to allow multi-scan history
pub async fn ensure_aws_resources_table(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Add sync_id column if missing
    let add_sync_id = r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM information_schema.columns 
                WHERE table_name = 'aws_resources' AND column_name = 'sync_id'
            ) THEN
                ALTER TABLE aws_resources ADD COLUMN sync_id UUID NULL;
            END IF;
        END $$;
    "#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        add_sync_id.to_string(),
    ))
    .await?;

    // Drop unique constraint on arn if exists to permit multiple versions across syncs
    let drop_unique_arn = r#"
        DO $$
        BEGIN
            IF EXISTS (
                SELECT 1 FROM pg_indexes 
                WHERE tablename = 'aws_resources' AND indexname = 'aws_resources_arn_key'
            ) THEN
                ALTER TABLE aws_resources DROP CONSTRAINT IF EXISTS aws_resources_arn_key;
            END IF;
        END $$;
    "#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        drop_unique_arn.to_string(),
    ))
    .await?;

    // Create index on sync_id
    let idx_sync =
        r#"CREATE INDEX IF NOT EXISTS idx_aws_resources_sync_id ON aws_resources(sync_id)"#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        idx_sync.to_string(),
    ))
    .await?;

    // Add composite unique to avoid duplicate rows for same resource within the same sync
    let add_unique = r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint c
                JOIN pg_class t ON c.conrelid = t.oid
                WHERE t.relname = 'aws_resources' AND c.conname = 'uniq_sync_arn'
            ) THEN
                ALTER TABLE aws_resources
                ADD CONSTRAINT uniq_sync_arn UNIQUE (sync_id, arn);
            END IF;
        END $$;
    "#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        add_unique.to_string(),
    ))
    .await?;

    // Add a more provider-agnostic uniqueness on natural key as well
    let add_unique2 = r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint c
                JOIN pg_class t ON c.conrelid = t.oid
                WHERE t.relname = 'aws_resources' AND c.conname = 'uniq_sync_resourcekey'
            ) THEN
                ALTER TABLE aws_resources
                ADD CONSTRAINT uniq_sync_resourcekey UNIQUE (sync_id, account_id, region, resource_type, resource_id);
            END IF;
        END $$;
    "#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        add_unique2.to_string(),
    ))
    .await?;

    // Helpful composite index for queries by type and sync
    let add_idx = r#"CREATE INDEX IF NOT EXISTS idx_aws_resources_type_sync ON aws_resources(resource_type, sync_id)"#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        add_idx.to_string(),
    ))
    .await?;

    Ok(())
}

/// Ensure the unified cloud_resources table exists and has expected indexes/constraints
/// This table stores multi-cloud normalized resources with a sync_id to support
/// multiple scans over time without collisions.
pub async fn ensure_cloud_resources_table(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Create table if it doesn't exist
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS cloud_resources (
            id UUID PRIMARY KEY,
            sync_id UUID NOT NULL,
            provider TEXT NOT NULL,
            account_id TEXT NOT NULL,
            region TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            arn_or_uri TEXT NULL,
            name TEXT NULL,
            tags JSONB NOT NULL DEFAULT '{}'::jsonb,
            resource_data JSONB NOT NULL DEFAULT '{}'::jsonb,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            last_refreshed TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#;

    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_table_sql.to_string(),
    ))
    .await?;

    // Helpful indexes for common queries
    let idx_sync =
        r#"CREATE INDEX IF NOT EXISTS idx_cloud_resources_sync_id ON cloud_resources(sync_id)"#;
    let idx_updated = r#"CREATE INDEX IF NOT EXISTS idx_cloud_resources_updated_at ON cloud_resources(updated_at DESC)"#;
    let idx_type_sync = r#"CREATE INDEX IF NOT EXISTS idx_cloud_resources_type_sync ON cloud_resources(resource_type, sync_id)"#;
    let idx_acct_region = r#"CREATE INDEX IF NOT EXISTS idx_cloud_resources_account_region ON cloud_resources(account_id, region)"#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        idx_sync.to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        idx_updated.to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        idx_type_sync.to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        idx_acct_region.to_string(),
    ))
    .await?;

    // Add a uniqueness guard to avoid duplicates for the same resource within a sync
    let add_unique = r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint c
                JOIN pg_class t ON c.conrelid = t.oid
                WHERE t.relname = 'cloud_resources' AND c.conname = 'uniq_cloud_sync_resource'
            ) THEN
                ALTER TABLE cloud_resources
                ADD CONSTRAINT uniq_cloud_sync_resource UNIQUE (sync_id, provider, account_id, region, resource_type, resource_id);
            END IF;
        END $$;
    "#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        add_unique.to_string(),
    ))
    .await?;

    // Ensure trigger to auto-update updated_at exists (uses set_updated_at created earlier)
    let drop_trigger =
        r#"DROP TRIGGER IF EXISTS trg_cloud_resources_updated_at ON cloud_resources"#;
    let create_trigger = r#"
        CREATE TRIGGER trg_cloud_resources_updated_at
        BEFORE UPDATE ON cloud_resources
        FOR EACH ROW
        EXECUTE PROCEDURE set_updated_at();
    "#;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        drop_trigger.to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_trigger.to_string(),
    ))
    .await?;

    Ok(())
}
