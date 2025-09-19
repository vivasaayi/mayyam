// Re-expose integration test helpers inside the crate so integration tests can import
// `crate::integration::helpers::*`. We include the existing files from tests/ tree.

pub mod harness {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/integration/helpers/harness.rs"));
}

pub mod auth {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/integration/helpers/auth.rs"));
}

pub mod server {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/integration/helpers/server.rs"));
}

pub mod kafka_test_helper {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/integration/helpers/kafka_test_helper.rs"));
}
