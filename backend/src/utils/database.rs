use sea_orm::{Database, DatabaseConnection, DbErr};
use tracing::{info, error};
use crate::config::Config;
use crate::errors::AppError;
use crate::models::database::Model as DatabaseConnectionModel;
use aes_gcm::{Aes256Gcm, Nonce, aead::{Aead, KeyInit}};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

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
        return Err(DbErr::Custom("No PostgreSQL database configuration found".to_string()));
    }
}

pub async fn connect_to_specific_postgres(config: &crate::config::PostgresConfig) -> Result<DatabaseConnection, DbErr> {
    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        config.username,
        config.password,
        config.host,
        config.port,
        config.database
    );

    info!("Connecting to PostgreSQL database: {}", config.name);
    let conn = Database::connect(database_url).await?;
    
    Ok(conn)
}

pub async fn connect_to_specific_mysql(config: &crate::config::MySQLConfig) -> Result<DatabaseConnection, DbErr> {
    let database_url = format!(
        "mysql://{}:{}@{}:{}/{}",
        config.username,
        config.password,
        config.host,
        config.port,
        config.database
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
        return Err(AppError::Config("Invalid encryption key length".to_string()));
    }
    
    // Decode base64
    let data = BASE64.decode(encrypted)
        .map_err(|e| AppError::Internal(format!("Base64 decoding error: {}", e)))?;
    
    if data.len() < 12 { // nonce is 12 bytes
        return Err(AppError::Internal("Invalid encrypted data".to_string()));
    }
    
    // Split nonce and ciphertext
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    // Create cipher
    let cipher = Aes256Gcm::new(encryption_key.into());
    
    // Decrypt
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| AppError::Internal(format!("Decryption error: {}", e)))?;
    
    String::from_utf8(plaintext)
        .map_err(|e| AppError::Internal(format!("UTF-8 decoding error: {}", e)))
}

/// Connect to a database based on a dynamic database connection model
pub async fn connect_to_dynamic_database(
    conn_model: &DatabaseConnectionModel, 
    config: &Config
) -> Result<DatabaseConnection, AppError> {
    // Decrypt password using the standalone decryption function
    let password = if let Some(encrypted_password) = &conn_model.password_encrypted {
        decrypt_password(encrypted_password, config)?
    } else {
        return Err(AppError::BadRequest("No password found for database connection".to_string()));
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
        },
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
        },
        _ => {
            return Err(AppError::BadRequest(format!(
                "Unsupported database type for dynamic connection: {}",
                conn_model.connection_type
            )));
        }
    };

    info!("Connecting to {} database: {}", conn_model.connection_type, conn_model.name);
    
    let conn = Database::connect(database_url)
        .await
        .map_err(|e| AppError::Database(e))?;
    
    Ok(conn)
}

/// Test a database connection
pub async fn test_database_connection(
    conn_model: &DatabaseConnectionModel,
    config: &Config
) -> Result<(), AppError> {
    let conn = connect_to_dynamic_database(conn_model, config).await?;
    
    // Try a simple query to test the connection
    match conn_model.connection_type.as_str() {
        "mysql" => {
            use sea_orm::{Statement, DbBackend, ConnectionTrait};
            let _result = conn.execute(Statement::from_string(
                DbBackend::MySql,
                "SELECT 1".to_string()
            )).await.map_err(AppError::Database)?;
        },
        "postgres" => {
            use sea_orm::{Statement, DbBackend, ConnectionTrait};
            let _result = conn.execute(Statement::from_string(
                DbBackend::Postgres,
                "SELECT 1".to_string()
            )).await.map_err(AppError::Database)?;
        },
        _ => {
            return Err(AppError::BadRequest(format!(
                "Connection testing not supported for database type: {}",
                conn_model.connection_type
            )));
        }
    }
    
    Ok(())
}