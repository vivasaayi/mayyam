use serde::{Deserialize, Serialize};

// S3-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3BucketInfo {
    pub bucket_name: String,
    pub creation_date: String,
    pub region: String,
    pub versioning_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3GetObjectRequest {
    pub bucket: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3PutObjectRequest {
    pub bucket: String,
    pub key: String,
    pub content_type: Option<String>,
    pub body: String,
}