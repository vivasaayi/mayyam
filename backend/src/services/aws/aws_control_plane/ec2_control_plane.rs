use std::sync::Arc;
use aws_sdk_ec2::Client as Ec2Client;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::aws_types::ec2::{Ec2InstanceInfo, Ec2InstanceVolumeModification, Ec2LaunchInstanceRequest, Ec2SecurityGroupRequest, Ec2VolumeRequest};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

// Control plane implementation for EC2
pub struct Ec2ControlPlane {
    aws_service: Arc<AwsService>,
}

impl Ec2ControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_instances(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_instances_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_instances_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_ec2_client_with_auth(profile, region, account_auth).await?;
        self.sync_instances_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_instances_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: Ec2Client) -> Result<Vec<AwsResourceModel>, AppError> {
        // Implementation using the provided client
        // For now returning sample data
        let mut instances = Vec::new();
        let instance = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: "EC2Instance".to_string(),
            resource_id: "i-0123456789abcdef0".to_string(),
            arn: format!("arn:aws:ec2:{}:{}:instance/i-0123456789abcdef0", region, account_id),
            name: Some("Sample EC2 Instance 1".to_string()),
            tags: json!({"Name": "Sample EC2 Instance 1", "Environment": "Development"}),
            resource_data: json!({
                "instance_id": "i-0123456789abcdef0",
                "instance_type": "t2.micro",
                "state": "running",
                "availability_zone": format!("{}a", region),
                "public_ip": "203.0.113.1",
                "private_ip": "10.0.0.1",
                "launch_time": "2023-05-01T12:00:00Z",
                "vpc_id": "vpc-0123abcd",
                "subnet_id": "subnet-0123abcd"
            }),
        };
        instances.push(instance);

        Ok(instances.into_iter().map(|i| i.into()).collect())
    }

    pub async fn launch_instances(&self, profile: Option<&str>, region: &str, request: &Ec2LaunchInstanceRequest) -> Result<Vec<Ec2InstanceInfo>, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation
        Ok(vec![
            Ec2InstanceInfo {
                instance_id: "i-1234567890abcdef0".to_string(),
                instance_type: request.instance_type.clone(),
                state: "pending".to_string(),
                availability_zone: format!("{}a", region),
                public_ip: None,
                private_ip: Some("172.31.16.100".to_string()),
                launch_time: chrono::Utc::now().to_rfc3339(),
                vpc_id: Some("vpc-1234567890abcdef0".to_string()),
                subnet_id: request.subnet_id.clone(),
            }
        ])
    }

    pub async fn start_instances(&self, profile: Option<&str>, region: &str, instance_ids: &[String]) -> Result<Vec<(String, String)>, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation returning instance ID and state pairs
        Ok(instance_ids.iter().map(|id| (id.clone(), "starting".to_string())).collect())
    }

    pub async fn stop_instances(&self, profile: Option<&str>, region: &str, instance_ids: &[String], force: bool) -> Result<Vec<(String, String)>, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation
        Ok(instance_ids.iter().map(|id| (id.clone(), "stopping".to_string())).collect())
    }

    pub async fn terminate_instances(&self, profile: Option<&str>, region: &str, instance_ids: &[String]) -> Result<Vec<(String, String)>, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation
        Ok(instance_ids.iter().map(|id| (id.clone(), "shutting-down".to_string())).collect())
    }

    pub async fn describe_instances(&self, profile: Option<&str>, region: &str, instance_ids: Option<&[String]>) -> Result<Vec<Ec2InstanceInfo>, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation
        Ok(vec![
            Ec2InstanceInfo {
                instance_id: "i-1234567890abcdef0".to_string(),
                instance_type: "t3.micro".to_string(),
                state: "running".to_string(),
                availability_zone: format!("{}a", region),
                public_ip: Some("54.123.45.67".to_string()),
                private_ip: Some("172.31.16.100".to_string()),
                launch_time: "2023-07-01T12:00:00Z".to_string(),
                vpc_id: Some("vpc-1234567890abcdef0".to_string()),
                subnet_id: Some("subnet-1234567890abcdef0".to_string()),
            }
        ])
    }

    pub async fn create_security_group(&self, profile: Option<&str>, region: &str, request: &Ec2SecurityGroupRequest) -> Result<String, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation returning security group ID
        Ok("sg-1234567890abcdef0".to_string())
    }

    pub async fn create_volume(&self, profile: Option<&str>, region: &str, request: &Ec2VolumeRequest) -> Result<String, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation returning volume ID
        Ok("vol-1234567890abcdef0".to_string())
    }

    pub async fn attach_volume(&self, profile: Option<&str>, region: &str, modification: &Ec2InstanceVolumeModification) -> Result<(), AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation
        Ok(())
    }

    pub async fn modify_instance_attribute(&self, profile: Option<&str>, region: &str, 
        instance_id: &str,
        attribute: &str,
        value: &str) -> Result<(), AppError> {
        
        let client = self.aws_service.create_ec2_client(profile, region).await?;
        
        // Mock implementation
        Ok(())
    }
}