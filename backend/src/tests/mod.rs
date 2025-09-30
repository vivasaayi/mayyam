// Legacy tests from `src/tests` have been migrated under the top-level
// `tests/` directory. This module now only exposes helper glue for integration
// suites that expect to reach `crate::tests::integration::helpers`.

#[cfg(feature = "integration-tests")]
pub mod integration;
