// Root module for AWS services
mod service;
mod control_plane;
mod data_plane;
#[path = "aws_client_factory.rs"]
mod client_factory;
mod cost;


// Re-export service structs
pub use service::AwsService;
pub use control_plane::AwsControlPlane;
pub use data_plane::AwsDataPlane;
pub use cost::AwsCostService;
pub use aws_data_plane::cloudwatch_data_plane::CloudWatchService;



pub mod aws_types;
pub mod aws_control_plane;
pub mod aws_data_plane;