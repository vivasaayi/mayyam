use serde::{Deserialize, Serialize};

// Common Request/Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncRequest {
    pub account_id: String,
    pub profile: Option<String>,
    pub region: String,
    pub resource_types: Option<Vec<String>>,
    // Authentication fields
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncResponse {
    pub summary: Vec<ResourceTypeSyncSummary>,
    pub total_resources: usize,
    pub sync_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTypeSyncSummary {
    pub resource_type: String,
    pub count: usize,
    pub status: String,
    pub details: Option<String>,
}