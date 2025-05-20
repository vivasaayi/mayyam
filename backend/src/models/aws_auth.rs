// Helper struct to pass authentication information when loading AWS SDK config
use crate::services::aws::aws_types::resource_sync::ResourceSyncRequest;

pub struct AccountAuthInfo {
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

impl From<&ResourceSyncRequest> for AccountAuthInfo {
    fn from(req: &ResourceSyncRequest) -> Self {
        Self {
            use_role: req.use_role,
            role_arn: req.role_arn.clone(),
            external_id: req.external_id.clone(),
            access_key_id: req.access_key_id.clone(),
            secret_access_key: req.secret_access_key.clone(),
        }
    }
}
