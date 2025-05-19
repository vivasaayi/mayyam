pub mod aws;
pub mod aws_account;
pub mod aws_analytics;
pub mod aws_dataplane;
pub mod database;
pub mod kafka;
pub mod user;

// Re-export commonly used services for backward compatibility
pub use aws::{
    AwsService, 
    AwsControlPlane, 
    AwsDataPlane,
    AwsCostService,
    CloudWatchService,
};