mod base;
mod metrics;
mod logs;
mod alarms;
mod types;

pub use base::CloudWatchService;
pub use metrics::CloudWatchMetrics;
pub use logs::CloudWatchLogs;
pub use alarms::CloudWatchAlarms;
pub use types::*;

// Re-export common types
pub use aws_sdk_cloudwatch::types::{
    Dimension,
    Metric,
    MetricDataQuery,
    MetricStat,
    Statistic,
    ComparisonOperator,
};
