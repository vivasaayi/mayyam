#![cfg(feature = "integration-tests")]

// Integration tests main entry point
mod integration {
    pub mod api_tests;
    pub mod helpers;
    pub mod kafka;
    pub mod llm;
}

// Re-export integration tests
pub use integration::api_tests::*;
pub use integration::helpers;
pub use integration::kafka::*;
pub use integration::llm::*;
