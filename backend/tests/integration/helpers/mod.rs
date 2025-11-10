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


// Expose helper modules for integration tests so paths like
// `crate::integration::helpers::...` resolve within the test crate.

pub mod auth;
pub mod harness;
pub mod kafka_test_helper;
pub mod server;

pub use harness::{get_aws_credentials, get_test_account_id, TestConfig, TestHarness};
pub use server::ensure_server;
