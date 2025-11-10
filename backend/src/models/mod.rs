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


pub mod cluster;
pub mod database;
pub mod user;

pub mod aws_auth;
pub mod aws_resource;
pub mod cloud_resource;

pub mod analytics;
pub mod aws_account;
pub mod data_source;
pub mod llm_model;
pub mod llm_provider;
pub mod prompt_template;
pub mod query_template;
pub mod sync_run;

// AWS Cost Analytics models
pub mod aws_cost_anomalies;
pub mod aws_cost_data;
pub mod aws_cost_insights;
pub mod aws_monthly_cost_aggregates;
pub mod cost_budget;

// MySQL Performance Analysis models
pub mod aurora_cluster;
pub mod slow_query_event;
pub mod query_fingerprint;
pub mod explain_plan;
pub mod ai_analysis;
pub mod mysql_performance_snapshot;

// Models module for data structures

pub use analytics::{Insight, InsightSeverity, Recommendation, RecommendationPriority};
