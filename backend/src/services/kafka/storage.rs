use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Gzip,
    Snappy,
    Lz4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMessage {
    pub offset: i64,
    pub timestamp: i64,
    pub key: Option<String>,
    pub value: String,
    pub headers: Option<Vec<(String, String)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    pub backup_id: String,
    pub topic: String,
    pub partition: i32,
    pub messages: Vec<BackupMessage>,
    pub checksum: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub backup_id: String,
    pub topic: String,
    pub partitions: Vec<i32>,
    pub created_at: String,
}

#[async_trait]
pub trait BackupStorage: Send + Sync {
    async fn store_backup(&self, data: &BackupData, compression: &CompressionType) -> Result<(), AppError>;
    async fn load_backup(&self, backup_id: &str, partition: i32) -> Result<BackupData, AppError>;
}

pub struct FileSystemStorage {
    base_path: PathBuf,
}

impl FileSystemStorage {
    pub fn new<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base_path: base.as_ref().to_path_buf(),
        }
    }

    pub fn get_metadata_path(&self, backup_id: &str) -> PathBuf {
        self.base_path.join(format!("{}.meta.json", backup_id))
    }
}

#[async_trait]
impl BackupStorage for FileSystemStorage {
    async fn store_backup(&self, data: &BackupData, _compression: &CompressionType) -> Result<(), AppError> {
        // Ensure dir exists
        if !self.base_path.exists() {
            fs::create_dir_all(&self.base_path)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create backup directory: {}", e)))?;
        }

        // Write partition file
        let part_path = self
            .base_path
            .join(format!("{}_part_{}.json", data.backup_id, data.partition));
        let json = serde_json::to_vec(data)
            .map_err(|e| AppError::Internal(format!("Failed to serialize backup data: {}", e)))?;
        let mut f = fs::File::create(&part_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create backup file: {}", e)))?;
        f.write_all(&json)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write backup file: {}", e)))?;

        // Update metadata (append partition if new)
        let meta_path = self.get_metadata_path(&data.backup_id);
        let mut metadata = if meta_path.exists() {
            let meta_bytes = fs::read(&meta_path)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read metadata: {}", e)))?;
            serde_json::from_slice::<BackupMetadata>(&meta_bytes)
                .map_err(|e| AppError::Internal(format!("Failed to parse metadata: {}", e)))?
        } else {
            BackupMetadata {
                backup_id: data.backup_id.clone(),
                topic: data.topic.clone(),
                partitions: vec![],
                created_at: data.created_at.clone(),
            }
        };
        if !metadata.partitions.contains(&data.partition) {
            metadata.partitions.push(data.partition);
        }
        let meta_json = serde_json::to_vec(&metadata)
            .map_err(|e| AppError::Internal(format!("Failed to serialize metadata: {}", e)))?;
        let mut mf = fs::File::create(&meta_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create metadata file: {}", e)))?;
        mf.write_all(&meta_json)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write metadata file: {}", e)))?;

        Ok(())
    }

    async fn load_backup(&self, backup_id: &str, partition: i32) -> Result<BackupData, AppError> {
        let part_path = self
            .base_path
            .join(format!("{}_part_{}.json", backup_id, partition));
        let bytes = fs::read(&part_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read backup file: {}", e)))?;
        let data: BackupData = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::Internal(format!("Failed to parse backup file: {}", e)))?;
        Ok(data)
    }
}
