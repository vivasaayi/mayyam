use serde::{Deserialize, Serialize};

// EC2-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ec2InstanceInfo {
    pub instance_id: String,
    pub instance_type: String,
    pub state: String,
    pub availability_zone: String,
    pub public_ip: Option<String>,
    pub private_ip: Option<String>,
    pub launch_time: String,
    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ec2LaunchInstanceRequest {
    pub image_id: String,
    pub instance_type: String,
    pub min_count: i32,
    pub max_count: i32,
    pub subnet_id: Option<String>,
    pub security_group_ids: Option<Vec<String>>,
    pub key_name: Option<String>,
    pub user_data: Option<String>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ec2SecurityGroupRule {
    pub ip_protocol: String,
    pub from_port: i32,
    pub to_port: i32,
    pub cidr_blocks: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ec2SecurityGroupRequest {
    pub group_name: String,
    pub description: String,
    pub vpc_id: String,
    pub ingress_rules: Vec<Ec2SecurityGroupRule>,
    pub egress_rules: Vec<Ec2SecurityGroupRule>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ec2VolumeRequest {
    pub availability_zone: String,
    pub volume_type: String,
    pub size: i32,
    pub iops: Option<i32>,
    pub encrypted: Option<bool>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ec2InstanceVolumeModification {
    pub instance_id: String,
    pub volume_id: String,
    pub device_name: String,
    pub delete_on_termination: Option<bool>,
}