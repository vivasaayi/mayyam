// Expose helper modules for integration tests so paths like
// `crate::integration::helpers::...` resolve within the test crate.

pub mod harness;
pub mod auth;
pub mod server;
pub mod kafka_test_helper;

pub use server::ensure_server;
pub use harness::{TestHarness, TestConfig, get_aws_credentials, get_test_account_id};
