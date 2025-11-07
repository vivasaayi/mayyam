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
