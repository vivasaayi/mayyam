pub mod user;
pub mod database;
pub mod cluster;

pub mod aws_resource;
pub mod aws_auth;

pub mod aws_account;
pub mod data_source;
pub mod llm_provider;
pub mod prompt_template;
pub mod query_template;
pub mod analytics;

// AWS Cost Analytics models
pub mod aws_cost_data;
pub mod aws_monthly_cost_aggregates;
pub mod aws_cost_anomalies;
pub mod aws_cost_insights;

// Models module for data structures

pub use analytics::{Insight, InsightSeverity, Recommendation, RecommendationPriority};
