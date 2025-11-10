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


#![cfg(feature = "integration-tests")]

// Integration test modules
pub mod api_tests;
pub mod aws_account_api_tests;
pub mod cost_analytics_api_tests;
pub mod database_api_tests;
pub mod helpers;
pub mod kafka;
pub mod kinesis_api_tests;
pub mod llm;
pub mod llm_integration_tests;
pub mod kubernetes_smoke_tests;
