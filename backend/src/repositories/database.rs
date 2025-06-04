use std::sync::Arc;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait};
use uuid::Uuid;
use chrono::Utc;
use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit},
    Nonce
};
use rand::{rngs::OsRng, RngCore};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::models::database::{self, Entity as DbConnection, Model as DbConnectionModel, ActiveModel as DbConnectionActiveModel};
use crate::errors::AppError;
use crate::config::Config;

pub struct DatabaseRepository {
    db: Arc<DatabaseConnection>,
    config: Config,
}

impl DatabaseRepository {
    pub fn new(db: Arc<DatabaseConnection>, config: Config) -> Self {
        Self { db, config }
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<DbConnectionModel>, AppError> {
        let connection = DbConnection::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(connection)
    }
    
    pub async fn find_by_name(&self, name: &str) -> Result<Option<DbConnectionModel>, AppError> {
        let connection = DbConnection::find()
            .filter(database::Column::Name.eq(name))
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(connection)
    }
    
    pub async fn find_all(&self) -> Result<Vec<DbConnectionModel>, AppError> {
        let connections = DbConnection::find()
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(connections)
    }
    
    pub async fn find_by_type(&self, connection_type: &str) -> Result<Vec<DbConnectionModel>, AppError> {
        let connections = DbConnection::find()
            .filter(database::Column::ConnectionType.eq(connection_type))
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;
        
        Ok(connections)
    }
    
    pub async fn create(&self, connection_data: &database::CreateDatabaseConnectionRequest, user_id: Uuid) -> Result<DbConnectionModel, AppError> {
        // Check if connection name already exists
        if let Some(_) = self.find_by_name(&connection_data.name).await? {
            return Err(AppError::Conflict(format!("Connection with name '{}' already exists", connection_data.name)));
        }
        
        // Encrypt password if provided
        let encrypted_password = match &connection_data.password {
            Some(pwd) if !pwd.is_empty() => Some(self.encrypt_password(pwd)?),
            _ => None,
        };
        
        let now = Utc::now();
        
        // Create new connection
        let connection = DbConnectionActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(connection_data.name.clone()),
            connection_type: Set(connection_data.connection_type.clone()),
            host: Set(connection_data.host.clone()),
            port: Set(connection_data.port),
            username: Set(connection_data.username.clone()),
            password_encrypted: Set(encrypted_password),
            database_name: Set(connection_data.database_name.clone()),
            ssl_mode: Set(connection_data.ssl_mode.clone()),
            cluster_mode: Set(connection_data.cluster_mode),
            created_by: Set(user_id),
            created_at: Set(now),
            updated_at: Set(now),
            last_connected_at: Set(None),
            connection_status: Set(Some("new".to_string())),
        };
        
        let connection = connection.insert(&*self.db).await.map_err(AppError::from)?;
        
        Ok(connection)
    }
    
    pub async fn update(&self, id: Uuid, connection_data: &database::CreateDatabaseConnectionRequest) -> Result<DbConnectionModel, AppError> {
        let connection = match self.find_by_id(id).await? {
            Some(conn) => conn,
            None => return Err(AppError::NotFound(format!("Database connection not found with ID: {}", id))),
        };
        
        // Check name uniqueness if it changed
        if connection.name != connection_data.name {
            if let Some(_) = self.find_by_name(&connection_data.name).await? {
                return Err(AppError::Conflict(format!("Connection with name '{}' already exists", connection_data.name)));
            }
        }
        
        // Encrypt password if provided
        let encrypted_password = match &connection_data.password {
            Some(pwd) if !pwd.is_empty() => Some(self.encrypt_password(pwd)?),
            _ => connection.password_encrypted.clone(),
        };
        
        let mut conn_active: DbConnectionActiveModel = connection.into();
        conn_active.name = Set(connection_data.name.clone());
        conn_active.connection_type = Set(connection_data.connection_type.clone());
        conn_active.host = Set(connection_data.host.clone());
        conn_active.port = Set(connection_data.port);
        conn_active.username = Set(connection_data.username.clone());
        conn_active.password_encrypted = Set(encrypted_password);
        conn_active.database_name = Set(connection_data.database_name.clone());
        conn_active.ssl_mode = Set(connection_data.ssl_mode.clone());
        conn_active.cluster_mode = Set(connection_data.cluster_mode);
        conn_active.updated_at = Set(Utc::now());
        
        let updated_conn = conn_active.update(&*self.db).await.map_err(AppError::from)?;
        
        Ok(updated_conn)
    }
    
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let connection = match self.find_by_id(id).await? {
            Some(conn) => conn,
            None => return Err(AppError::NotFound(format!("Database connection not found with ID: {}", id))),
        };
        
        let conn_active: DbConnectionActiveModel = connection.into();
        conn_active.delete(&*self.db).await.map_err(AppError::from)?;
        
        Ok(())
    }
    
    pub async fn update_connection_status(&self, id: Uuid, status: &str) -> Result<DbConnectionModel, AppError> {
        let connection = match self.find_by_id(id).await? {
            Some(conn) => conn,
            None => return Err(AppError::NotFound(format!("Database connection not found with ID: {}", id))),
        };
        
        let mut conn_active: DbConnectionActiveModel = connection.into();
        conn_active.connection_status = Set(Some(status.to_string()));
        
        if status == "connected" {
            conn_active.last_connected_at = Set(Some(Utc::now()));
        }
        
        conn_active.updated_at = Set(Utc::now());
        
        let updated_conn = conn_active.update(&*self.db).await.map_err(AppError::from)?;
        
        Ok(updated_conn)
    }
    
    // Helper method to encrypt password
    fn encrypt_password(&self, password: &str) -> Result<String, AppError> {
        let encryption_key = self.config.security.encryption_key.as_bytes();
        if encryption_key.len() != 32 {
            return Err(AppError::Config("Invalid encryption key length".to_string()));
        }
        
        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(encryption_key)
            .map_err(|e| AppError::Internal(format!("Encryption error: {}", e)))?;
        
        // Generate random nonce
        let mut nonce_bytes = [0u8; 12]; // AES-GCM uses 12-byte nonces
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Encrypt
        let ciphertext = cipher.encrypt(nonce, password.as_bytes())
            .map_err(|e| AppError::Internal(format!("Encryption error: {}", e)))?;
        
        // Combine nonce + ciphertext and encode with base64
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        
        Ok(BASE64.encode(combined))
    }
    
    // Helper method to decrypt password
    pub fn decrypt_password(&self, encrypted: &str) -> Result<String, AppError> {
        let encryption_key = self.config.security.encryption_key.as_bytes();
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
        let cipher = Aes256Gcm::new_from_slice(encryption_key)
            .map_err(|e| AppError::Internal(format!("Encryption error: {}", e)))?;
        
        // Decrypt
        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Internal(format!("Decryption error: {}", e)))?;
        
        String::from_utf8(plaintext)
            .map_err(|e| AppError::Internal(format!("UTF-8 decoding error: {}", e)))
    }
}