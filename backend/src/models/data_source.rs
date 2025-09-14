use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;
use std::str::FromStr;

/// Data source configuration entity for managing different data source types
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "data_sources")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub data_source_type: String,
    pub resource_type: String, // e.g., "DynamoDB", "Kubernetes", "RDS"
    pub source_type: String,   // e.g., "CloudWatch", "Dynatrace", "Splunk", "Prometheus"
    pub connection_config: Json, // Connection configuration specific to source type
    pub metric_config: Option<Json>, // Configuration for metric collection
    pub thresholds: Option<Json>, // Threshold configurations
    pub enabled: bool, // Indicates if the data source is enabled
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// Domain model for data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceDomain {
    pub id: Uuid,
    pub name: String,
    pub resource_type: String,
    pub source_type: String,
    pub connection_config: serde_json::Value,
    pub metric_definitions: Vec<MetricDefinition>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Metric definition structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub unit: String,
    pub metric_type: MetricType,
    pub purpose: String, // What this metric is used for in analysis
    pub collection_method: String, // How to collect this metric
    pub thresholds: Option<MetricThresholds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricThresholds {
    pub warning: Option<f64>,
    pub critical: Option<f64>,
    pub optimal_min: Option<f64>,
    pub optimal_max: Option<f64>,
}

/// DTO for creating new data source
#[derive(Debug, Deserialize)]
pub struct DataSourceCreateDto {
    pub name: String,
    pub resource_type: String,
    pub source_type: String,
    pub connection_config: serde_json::Value,
    pub metric_definitions: Vec<MetricDefinition>,
    pub enabled: Option<bool>,
}

/// DTO for updating data source
#[derive(Debug, Deserialize)]
pub struct DataSourceUpdateDto {
    pub name: Option<String>,
    pub resource_type: Option<String>,
    pub source_type: Option<String>,
    pub connection_config: Option<serde_json::Value>,
    pub metric_definitions: Option<Vec<MetricDefinition>>,
    pub enabled: Option<bool>,
}

/// DTO for API responses
#[derive(Debug, Serialize)]
pub struct DataSourceDto {
    pub id: Uuid,
    pub name: String,
    pub resource_type: String,
    pub source_type: String,
    pub connection_config: serde_json::Value,
    pub metric_definitions: Vec<MetricDefinition>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Add missing enum types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataSourceType {
    CloudWatch,
    Dynatrace,
    Splunk,
    Prometheus,
    CustomMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResourceType {
    DynamoDB,
    Kubernetes,
    RDS,
    ECS,
    Lambda,
    S3,
    CloudFront,
    ALB,
    ELB,
    Kinesis,
    SQS,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SourceType {
    CloudWatch,
    Dynatrace,
    Splunk,
    Prometheus,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataSourceStatus {
    Active,
    Inactive,
    Error,
    Pending,
}

// Add missing request/response DTOs
#[derive(Debug, Deserialize)]
pub struct CreateDataSourceRequest {
    pub name: String,
    pub resource_type: ResourceType,
    pub source_type: SourceType,
    pub connection_config: serde_json::Value,
    pub metric_definitions: Vec<MetricDefinition>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
    pub data_source_type: DataSourceType,
    pub metric_config: Option<serde_json::Value>,
    pub thresholds: Option<MetricThresholds>,
    pub status: Option<DataSourceStatus>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDataSourceRequest {
    pub name: Option<String>,
    pub resource_type: Option<ResourceType>,
    pub source_type: Option<SourceType>,
    pub connection_config: Option<serde_json::Value>,
    pub metric_definitions: Option<Vec<MetricDefinition>>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
    pub data_source_type: Option<DataSourceType>,
    pub metric_config: Option<serde_json::Value>,
    pub thresholds: Option<MetricThresholds>,
    pub status: Option<DataSourceStatus>,
}

#[derive(Debug, Deserialize)]
pub struct DataSourceQueryParams {
    pub name: Option<String>,
    pub resource_type: Option<ResourceType>,
    pub source_type: Option<SourceType>,
    pub enabled: Option<bool>,
    pub status: Option<DataSourceStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDataSourceDto {
    pub name: String,
    pub resource_type: ResourceType,
    pub source_type: SourceType,
    pub connection_config: serde_json::Value,
    pub metric_definitions: Vec<MetricDefinition>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
    pub data_source_type: DataSourceType,
    pub metric_config: Option<serde_json::Value>,
    pub thresholds: Option<MetricThresholds>,
    pub status: Option<DataSourceStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateDataSourceDto {
    pub name: Option<String>,
    pub resource_type: Option<ResourceType>,
    pub source_type: Option<SourceType>,
    pub connection_config: Option<serde_json::Value>,
    pub metric_definitions: Option<Vec<MetricDefinition>>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
    pub data_source_type: Option<DataSourceType>,
    pub metric_config: Option<serde_json::Value>,
    pub thresholds: Option<MetricThresholds>,
    pub status: Option<DataSourceStatus>,
}

#[derive(Debug, Serialize)]
pub struct DataSourceResponseDto {
    pub id: Uuid,
    pub name: String,
    pub resource_type: ResourceType,
    pub source_type: SourceType,
    pub connection_config: serde_json::Value,
    pub metric_definitions: Vec<MetricDefinition>,
    pub enabled: bool,
    pub description: Option<String>,
    pub data_source_type: DataSourceType,
    pub metric_config: Option<serde_json::Value>,
    pub thresholds: Option<MetricThresholds>,
    pub status: DataSourceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// String conversions for database storage
impl std::fmt::Display for DataSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSourceType::CloudWatch => write!(f, "CloudWatch"),
            DataSourceType::Dynatrace => write!(f, "Dynatrace"),
            DataSourceType::Splunk => write!(f, "Splunk"),
            DataSourceType::Prometheus => write!(f, "Prometheus"),
            DataSourceType::CustomMetrics => write!(f, "CustomMetrics"),
        }
    }
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::DynamoDB => write!(f, "DynamoDB"),
            ResourceType::Kubernetes => write!(f, "Kubernetes"),
            ResourceType::RDS => write!(f, "RDS"),
            ResourceType::ECS => write!(f, "ECS"),
            ResourceType::Lambda => write!(f, "Lambda"),
            ResourceType::S3 => write!(f, "S3"),
            ResourceType::CloudFront => write!(f, "CloudFront"),
            ResourceType::ALB => write!(f, "ALB"),
            ResourceType::ELB => write!(f, "ELB"),
            ResourceType::Kinesis => write!(f, "Kinesis"),
            ResourceType::SQS => write!(f, "SQS"),
            ResourceType::Custom => write!(f, "Custom"),
        }
    }
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::CloudWatch => write!(f, "CloudWatch"),
            SourceType::Dynatrace => write!(f, "Dynatrace"),
            SourceType::Splunk => write!(f, "Splunk"),
            SourceType::Prometheus => write!(f, "Prometheus"),
            SourceType::Custom => write!(f, "Custom"),
        }
    }
}

impl std::fmt::Display for DataSourceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSourceStatus::Active => write!(f, "Active"),
            DataSourceStatus::Inactive => write!(f, "Inactive"),
            DataSourceStatus::Error => write!(f, "Error"),
            DataSourceStatus::Pending => write!(f, "Pending"),
        }
    }
}

// Conversions
impl From<Model> for DataSourceDomain {
    fn from(entity: Model) -> Self {
        let metric_definitions: Vec<MetricDefinition> = entity.metric_config
            .as_ref()
            .and_then(|config| serde_json::from_value(config.clone()).ok())
            .unwrap_or_default();

        Self {
            id: entity.id,
            name: entity.name,
            resource_type: entity.resource_type,
            source_type: entity.source_type,
            connection_config: entity.connection_config,
            metric_definitions,
            enabled: entity.enabled,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }
}

impl From<DataSourceDomain> for DataSourceDto {
    fn from(domain: DataSourceDomain) -> Self {
        Self {
            id: domain.id,
            name: domain.name,
            resource_type: domain.resource_type,
            source_type: domain.source_type,
            connection_config: domain.connection_config,
            metric_definitions: domain.metric_definitions,
            enabled: domain.enabled,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

impl From<DataSourceCreateDto> for ActiveModel {
    fn from(dto: DataSourceCreateDto) -> Self {
        let now = Utc::now();
        let metric_definitions_json = serde_json::to_value(dto.metric_definitions)
            .unwrap_or(serde_json::Value::Array(vec![]));

        Self {
            id: Set(Uuid::new_v4()),
            name: Set(dto.name),
            description: Set(None), // Add default value
            data_source_type: Set(dto.resource_type.clone()), // Map resource_type to data_source_type
            resource_type: Set(dto.resource_type),
            source_type: Set(dto.source_type),
            connection_config: Set(dto.connection_config),
            metric_config: Set(Some(metric_definitions_json)), // Map metric_definitions to metric_config
            thresholds: Set(None), // Add default value
            status: Set("active".to_string()), // Add default status
            created_at: Set(now),
            updated_at: Set(now),
            enabled: todo!(),
        }
    }
}

// Conversion from Model to DataSourceResponseDto
impl From<Model> for DataSourceResponseDto {
    fn from(entity: Model) -> Self {
        // Parse the string fields back to enums
        let resource_type: ResourceType = serde_json::from_str(&format!("\"{}\"", entity.resource_type))
            .unwrap_or(ResourceType::Custom);
        let source_type: SourceType = serde_json::from_str(&format!("\"{}\"", entity.source_type))
            .unwrap_or(SourceType::Custom);
        let data_source_type: DataSourceType = serde_json::from_str(&format!("\"{}\"", entity.data_source_type))
            .unwrap_or(DataSourceType::CustomMetrics);
        let status: DataSourceStatus = serde_json::from_str(&format!("\"{}\"", entity.status))
            .unwrap_or(DataSourceStatus::Active);

        // Extract metric_definitions from metric_config if available
        let metric_definitions: Vec<MetricDefinition> = entity.metric_config
            .as_ref()
            .and_then(|config| serde_json::from_value(config.clone()).ok())
            .unwrap_or_default();

        // Extract thresholds if available
        let thresholds: Option<MetricThresholds> = entity.thresholds
            .as_ref()
            .and_then(|t| serde_json::from_value(t.clone()).ok());

        Self {
            id: entity.id,
            name: entity.name,
            resource_type,
            source_type,
            connection_config: entity.connection_config,
            metric_definitions,
            enabled: entity.enabled,
            description: entity.description,
            data_source_type,
            metric_config: entity.metric_config,
            thresholds,
            status,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }
}

impl From<&Model> for DataSourceResponseDto {
    fn from(entity: &Model) -> Self {
        entity.clone().into()
    }
}

impl SourceType {
    pub fn from_str_case_insensitive(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cloudwatch" => Some(SourceType::CloudWatch),
            "dynatrace" => Some(SourceType::Dynatrace),
            "splunk" => Some(SourceType::Splunk),
            "prometheus" => Some(SourceType::Prometheus),
            "custom" => Some(SourceType::Custom),
            _ => None,
        }
    }
}

impl ResourceType {
    pub fn from_str_case_insensitive(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dynamodb" => Some(ResourceType::DynamoDB),
            "kubernetes" => Some(ResourceType::Kubernetes),
            "rds" => Some(ResourceType::RDS),
            "ecs" => Some(ResourceType::ECS),
            "lambda" => Some(ResourceType::Lambda),
            "s3" => Some(ResourceType::S3),
            "cloudfront" => Some(ResourceType::CloudFront),
            "alb" => Some(ResourceType::ALB),
            "elb" => Some(ResourceType::ELB),
            "custom" => Some(ResourceType::Custom),
            _ => None,
        }
    }
}
