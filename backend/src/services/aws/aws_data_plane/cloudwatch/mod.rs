mod alarms;
mod base;
mod logs;
mod metrics;
mod types;

pub use alarms::CloudWatchAlarms;
pub use base::CloudWatchService;
pub use logs::CloudWatchLogs;
pub use metrics::CloudWatchMetrics;
pub use types::*;

// Re-export common types
pub use aws_sdk_cloudwatch::types::{
    ComparisonOperator, Dimension, Metric, MetricDataQuery, MetricStat, Statistic,
};
