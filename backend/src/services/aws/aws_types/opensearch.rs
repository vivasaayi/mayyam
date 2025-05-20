use serde::{Deserialize, Serialize};

// OpenSearch Types 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSearchClusterHealthRequest {
    pub domain_name: String,
    pub index: Option<String>,
    pub level: Option<String>,
    pub wait_for_status: Option<String>,
    pub timeout: Option<String>,
}