// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "aws_resources")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub sync_id: Option<Uuid>,
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
    // IAM Resources
    IamUser,
    IamRole,
    IamPolicy,
    IamGroup,
    // VPC & Networking Resources
    Vpc,
    Subnet,
    SecurityGroup,
    InternetGateway,
    NatGateway,
    RouteTable,
    NetworkAcl,
    // Load Balancing & CDN Resources
    Alb,
    Nlb,
    Elb,
    CloudFrontDistribution,
    ApiGatewayRestApi,
    ApiGatewayStage,
    ApiGatewayResource,
    ApiGatewayMethod,
    // Storage & Content Delivery Resources
    EbsVolume,
    EbsSnapshot,
    EfsFileSystem,
    // Security & Compliance Resources
    KmsKey,
    AcmCertificate,
    CloudTrailTrail,
    ConfigRule,
    // Container & Serverless Resources
    EcsCluster,
    EcsService,
    EcsTask,
    EksCluster,
    FargateProfile,
    AppRunnerService,
    BatchComputeEnv,
    // Management & Monitoring Resources
    CloudWatchAlarm,
    CloudWatchDashboard,
    SsmDocument,
    // Application Integration Resources
    EventBridgeRule,
    StepFunction,
    SesIdentity,
    AppSyncApi,
    // Analytics & Big Data Resources
    RedshiftCluster,
    EmrCluster,
    AthenaWorkgroup,
    GlueDatabase,
    KinesisAnalyticsApp,
    // Edge Computing Resources
    WafWebAcl,
    GlobalAccelerator,
    // Backup & DR Resources
    BackupVault,
    BackupPlan,
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
            // IAM Resources
            AwsResourceType::IamUser => "IamUser".to_string(),
            AwsResourceType::IamRole => "IamRole".to_string(),
            AwsResourceType::IamPolicy => "IamPolicy".to_string(),
            AwsResourceType::IamGroup => "IamGroup".to_string(),
            // VPC & Networking Resources
            AwsResourceType::Vpc => "Vpc".to_string(),
            AwsResourceType::Subnet => "Subnet".to_string(),
            AwsResourceType::SecurityGroup => "SecurityGroup".to_string(),
            AwsResourceType::InternetGateway => "InternetGateway".to_string(),
            AwsResourceType::NatGateway => "NatGateway".to_string(),
            AwsResourceType::RouteTable => "RouteTable".to_string(),
            AwsResourceType::NetworkAcl => "NetworkAcl".to_string(),
            // Load Balancing & CDN Resources
            AwsResourceType::Alb => "Alb".to_string(),
            AwsResourceType::Nlb => "Nlb".to_string(),
            AwsResourceType::Elb => "Elb".to_string(),
            AwsResourceType::CloudFrontDistribution => "CloudFrontDistribution".to_string(),
            AwsResourceType::ApiGatewayRestApi => "ApiGatewayRestApi".to_string(),
            AwsResourceType::ApiGatewayStage => "ApiGatewayStage".to_string(),
            AwsResourceType::ApiGatewayResource => "ApiGatewayResource".to_string(),
            AwsResourceType::ApiGatewayMethod => "ApiGatewayMethod".to_string(),
            // Storage & Content Delivery Resources
            AwsResourceType::EbsVolume => "EbsVolume".to_string(),
            AwsResourceType::EbsSnapshot => "EbsSnapshot".to_string(),
            AwsResourceType::EfsFileSystem => "EfsFileSystem".to_string(),
            // Security & Compliance Resources
            AwsResourceType::KmsKey => "KmsKey".to_string(),
            AwsResourceType::AcmCertificate => "AcmCertificate".to_string(),
            AwsResourceType::CloudTrailTrail => "CloudTrailTrail".to_string(),
            AwsResourceType::ConfigRule => "ConfigRule".to_string(),
            // Container & Serverless Resources
            AwsResourceType::EcsCluster => "EcsCluster".to_string(),
            AwsResourceType::EcsService => "EcsService".to_string(),
            AwsResourceType::EcsTask => "EcsTask".to_string(),
            AwsResourceType::EksCluster => "EksCluster".to_string(),
            AwsResourceType::FargateProfile => "FargateProfile".to_string(),
            AwsResourceType::AppRunnerService => "AppRunnerService".to_string(),
            AwsResourceType::BatchComputeEnv => "BatchComputeEnv".to_string(),
            // Management & Monitoring Resources
            AwsResourceType::CloudWatchAlarm => "CloudWatchAlarm".to_string(),
            AwsResourceType::CloudWatchDashboard => "CloudWatchDashboard".to_string(),
            AwsResourceType::SsmDocument => "SsmDocument".to_string(),
            // Application Integration Resources
            AwsResourceType::EventBridgeRule => "EventBridgeRule".to_string(),
            AwsResourceType::StepFunction => "StepFunction".to_string(),
            AwsResourceType::SesIdentity => "SesIdentity".to_string(),
            AwsResourceType::AppSyncApi => "AppSyncApi".to_string(),
            // Analytics & Big Data Resources
            AwsResourceType::RedshiftCluster => "RedshiftCluster".to_string(),
            AwsResourceType::EmrCluster => "EmrCluster".to_string(),
            AwsResourceType::AthenaWorkgroup => "AthenaWorkgroup".to_string(),
            AwsResourceType::GlueDatabase => "GlueDatabase".to_string(),
            AwsResourceType::KinesisAnalyticsApp => "KinesisAnalyticsApp".to_string(),
            // Edge Computing Resources
            AwsResourceType::WafWebAcl => "WafWebAcl".to_string(),
            AwsResourceType::GlobalAccelerator => "GlobalAccelerator".to_string(),
            // Backup & DR Resources
            AwsResourceType::BackupVault => "BackupVault".to_string(),
            AwsResourceType::BackupPlan => "BackupPlan".to_string(),
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
            // IAM Resources
            "IamUser" => AwsResourceType::IamUser,
            "IamRole" => AwsResourceType::IamRole,
            "IamPolicy" => AwsResourceType::IamPolicy,
            "IamGroup" => AwsResourceType::IamGroup,
            // VPC & Networking Resources
            "Vpc" => AwsResourceType::Vpc,
            "Subnet" => AwsResourceType::Subnet,
            "SecurityGroup" => AwsResourceType::SecurityGroup,
            "InternetGateway" => AwsResourceType::InternetGateway,
            "NatGateway" => AwsResourceType::NatGateway,
            "RouteTable" => AwsResourceType::RouteTable,
            "NetworkAcl" => AwsResourceType::NetworkAcl,
            // Load Balancing & CDN Resources
            "Alb" => AwsResourceType::Alb,
            "Nlb" => AwsResourceType::Nlb,
            "Elb" => AwsResourceType::Elb,
            "CloudFrontDistribution" => AwsResourceType::CloudFrontDistribution,
            "ApiGatewayRestApi" => AwsResourceType::ApiGatewayRestApi,
            "ApiGatewayStage" => AwsResourceType::ApiGatewayStage,
            "ApiGatewayResource" => AwsResourceType::ApiGatewayResource,
            "ApiGatewayMethod" => AwsResourceType::ApiGatewayMethod,
            // Storage & Content Delivery Resources
            "EbsVolume" => AwsResourceType::EbsVolume,
            "EbsSnapshot" => AwsResourceType::EbsSnapshot,
            "EfsFileSystem" => AwsResourceType::EfsFileSystem,
            // Security & Compliance Resources
            "KmsKey" => AwsResourceType::KmsKey,
            "AcmCertificate" => AwsResourceType::AcmCertificate,
            "CloudTrailTrail" => AwsResourceType::CloudTrailTrail,
            "ConfigRule" => AwsResourceType::ConfigRule,
            // Container & Serverless Resources
            "EcsCluster" => AwsResourceType::EcsCluster,
            "EcsService" => AwsResourceType::EcsService,
            "EcsTask" => AwsResourceType::EcsTask,
            "EksCluster" => AwsResourceType::EksCluster,
            "FargateProfile" => AwsResourceType::FargateProfile,
            "AppRunnerService" => AwsResourceType::AppRunnerService,
            "BatchComputeEnv" => AwsResourceType::BatchComputeEnv,
            // Management & Monitoring Resources
            "CloudWatchAlarm" => AwsResourceType::CloudWatchAlarm,
            "CloudWatchDashboard" => AwsResourceType::CloudWatchDashboard,
            "SsmDocument" => AwsResourceType::SsmDocument,
            // Application Integration Resources
            "EventBridgeRule" => AwsResourceType::EventBridgeRule,
            "StepFunction" => AwsResourceType::StepFunction,
            "SesIdentity" => AwsResourceType::SesIdentity,
            "AppSyncApi" => AwsResourceType::AppSyncApi,
            // Analytics & Big Data Resources
            "RedshiftCluster" => AwsResourceType::RedshiftCluster,
            "EmrCluster" => AwsResourceType::EmrCluster,
            "AthenaWorkgroup" => AwsResourceType::AthenaWorkgroup,
            "GlueDatabase" => AwsResourceType::GlueDatabase,
            "KinesisAnalyticsApp" => AwsResourceType::KinesisAnalyticsApp,
            // Edge Computing Resources
            "WafWebAcl" => AwsResourceType::WafWebAcl,
            "GlobalAccelerator" => AwsResourceType::GlobalAccelerator,
            // Backup & DR Resources
            "BackupVault" => AwsResourceType::BackupVault,
            "BackupPlan" => AwsResourceType::BackupPlan,
            _ => panic!("Unknown resource type: {}", s),
        }
    }
}

// DTOs for AWS resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsResourceDto {
    pub id: Option<Uuid>,
    pub sync_id: Option<Uuid>,
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
    pub sync_id: Option<Uuid>,
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

impl From<AwsResourceDto> for Model {
    fn from(dto: AwsResourceDto) -> Self {
        let now = Utc::now();
        Self {
            id: dto.id.unwrap_or_else(Uuid::new_v4),
            sync_id: dto.sync_id,
            account_id: dto.account_id,
            profile: dto.profile,
            region: dto.region,
            resource_type: dto.resource_type,
            resource_id: dto.resource_id,
            arn: dto.arn,
            name: dto.name,
            tags: dto.tags,
            resource_data: dto.resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now,
        }
    }
}

impl From<Model> for AwsResourceDto {
    fn from(model: Model) -> Self {
        Self {
            id: Some(model.id),
            sync_id: model.sync_id,
            account_id: model.account_id,
            profile: model.profile,
            region: model.region,
            resource_type: model.resource_type,
            resource_id: model.resource_id,
            arn: model.arn,
            name: model.name,
            tags: model.tags,
            resource_data: model.resource_data,
        }
    }
}
