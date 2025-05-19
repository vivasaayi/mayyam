// Root module for AWS services
mod types;
mod service;
mod control_plane;
mod data_plane;
#[path = "aws_client_factory.rs"]
mod client_factory;
mod cost;
mod cloudwatch;

// Re-export common types
pub use types::*;

// Re-export service structs
pub use service::AwsService;
pub use control_plane::AwsControlPlane;
pub use data_plane::AwsDataPlane;
pub use cost::AwsCostService;
pub use cloudwatch::CloudWatchService;

// Service-specific modules
pub mod ec2;
pub mod s3;
pub mod rds;
pub mod dynamodb;
pub mod kinesis;
pub mod sqs;
pub mod sns;
pub mod lambda;
pub mod elasticache;
pub mod opensearch;
