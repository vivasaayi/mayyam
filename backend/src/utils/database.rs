use sea_orm::{Database, DatabaseConnection, DbErr};
use tracing::{info, error};
use crate::config::Config;
use crate::errors::AppError;

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
    let conn = Database::connect(database_url).await?;
    
    Ok(conn)
}