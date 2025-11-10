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
