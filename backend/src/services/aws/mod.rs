// Root module for AWS services
#[path = "aws_client_factory.rs"]
mod client_factory;
mod control_plane;
mod data_plane;
mod service;

// Re-export service structs
pub use aws_data_plane::cost_explorer::AwsCostService;
pub use control_plane::AwsControlPlane;
pub use data_plane::AwsDataPlane;
pub use service::AwsService;
mod aws_config_service;
pub mod aws_control_plane;
pub mod aws_data_plane;
pub mod aws_types;
