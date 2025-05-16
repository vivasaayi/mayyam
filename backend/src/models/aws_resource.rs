use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "aws_resources")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub account_id: String,
    pub profile: Option<String>,
    pub region: String,
    pub resource_type: String,
    pub resource_id: String,
    pub arn: String,
    pub name: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub tags: serde_json::Value,
    #[sea_orm(column_type = "Json")]
    pub resource_data: serde_json::Value,
    #[sea_orm(column_type = "Timestamp")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(column_type = "Timestamp")]
    pub updated_at: DateTime<Utc>,
    #[sea_orm(column_type = "Timestamp")]
    pub last_refreshed: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Enum for AWS resource types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AwsResourceType {
    EC2Instance,
    S3Bucket,
    RdsInstance,
    DynamoDbTable,
    KinesisStream,
    SqsQueue,
    SnsTopics,
    LambdaFunction,
    ElasticacheCluster,
    OpenSearchDomain,
}

impl ToString for AwsResourceType {
    fn to_string(&self) -> String {
        match self {
            AwsResourceType::EC2Instance => "EC2Instance".to_string(),
            AwsResourceType::S3Bucket => "S3Bucket".to_string(),
            AwsResourceType::RdsInstance => "RdsInstance".to_string(),
            AwsResourceType::DynamoDbTable => "DynamoDbTable".to_string(),
            AwsResourceType::KinesisStream => "KinesisStream".to_string(),
            AwsResourceType::SqsQueue => "SqsQueue".to_string(),
            AwsResourceType::SnsTopics => "SnsTopic".to_string(),
            AwsResourceType::LambdaFunction => "LambdaFunction".to_string(),
            AwsResourceType::ElasticacheCluster => "ElasticacheCluster".to_string(),
            AwsResourceType::OpenSearchDomain => "OpenSearchDomain".to_string(),
        }
    }
}

impl From<&str> for AwsResourceType {
    fn from(s: &str) -> Self {
        match s {
            "EC2Instance" => AwsResourceType::EC2Instance,
            "S3Bucket" => AwsResourceType::S3Bucket,
            "RdsInstance" => AwsResourceType::RdsInstance,
            "DynamoDbTable" => AwsResourceType::DynamoDbTable,
            "KinesisStream" => AwsResourceType::KinesisStream,
            "SqsQueue" => AwsResourceType::SqsQueue,
            "SnsTopic" => AwsResourceType::SnsTopics,
            "LambdaFunction" => AwsResourceType::LambdaFunction,
            "ElasticacheCluster" => AwsResourceType::ElasticacheCluster,
            "OpenSearchDomain" => AwsResourceType::OpenSearchDomain,
            _ => panic!("Unknown resource type: {}", s),
        }
    }
}

// DTOs for AWS resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsResourceDto {
    pub id: Option<Uuid>,
    pub account_id: String,
    pub profile: Option<String>,
    pub region: String,
    pub resource_type: String,
    pub resource_id: String,
    pub arn: String,
    pub name: Option<String>,
    pub tags: serde_json::Value,
    pub resource_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsResourceQuery {
    pub account_id: Option<String>,
    pub profile: Option<String>,
    pub region: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub name: Option<String>,
    pub tag_key: Option<String>,
    pub tag_value: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsResourcePage {
    pub resources: Vec<Model>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}