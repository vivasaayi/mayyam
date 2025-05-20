use serde::{Deserialize, Serialize};

// RDS-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsInstanceInfo {
    pub db_instance_identifier: String,
    pub engine: String,
    pub engine_version: String,
    pub instance_class: String,
    pub allocated_storage: i32,
    pub endpoint: Option<RdsEndpoint>,
    pub status: String,
    pub availability_zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsEndpoint {
    pub address: String,
    pub port: i32,
    pub hosted_zone_id: String,
}