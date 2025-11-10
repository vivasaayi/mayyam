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


pub mod ai_analysis_service;
pub mod aws;
pub mod aws_account;
pub mod aws_cost_analytics;
pub mod aws_dataplane;
pub mod budget_service;
pub mod cost_categories;
pub mod database;
pub mod explain_plan_service;
pub mod kafka;
pub mod mysql_performance_service;
pub mod query_fingerprinting_service;
pub mod resource_cost_enrichment;
pub mod slow_query_ingestion_service;
pub mod user;

pub mod analytics;
pub mod cloudwatch_scraper;
pub mod data_collection;
pub mod llm;
pub mod llm_provider;
pub mod metric_streams_parser;

// Re-export commonly used services for backward compatibility
pub use aws::AwsService;

pub mod kubernetes;
