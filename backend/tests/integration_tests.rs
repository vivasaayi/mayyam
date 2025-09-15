#![cfg(feature = "integration-tests")]

// Integration tests main entry point
mod integration {
    pub mod api_tests;
}

// Re-export integration tests
pub use integration::api_tests::*;
