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


mod base;
mod cost_usage;
mod dimensions;
mod forecasting;

pub use base::AwsCostService;
pub use cost_usage::{CostAndUsage, DatePreset};
pub use dimensions::DimensionValues;
pub use forecasting::CostForecasting;

// Re-export the types that are used in public interfaces
pub use aws_sdk_costexplorer::types::{
    Context, DateInterval, DimensionValues as AwsDimensionValues, Expression, Granularity,
    GroupDefinition,
};

// Example usage for your use case:
/*
// Get cost comparison between last Friday and this Friday by usage type
let last_friday = DatePreset::Custom("2025-05-16".to_string(), "2025-05-16".to_string());
let this_friday = DatePreset::Custom("2025-05-23".to_string(), "2025-05-23".to_string());

let group_by = vec![
    GroupDefinition::builder()
        .key("USAGE_TYPE")
        .type_("DIMENSION")
        .build()
];

let comparison = cost_service.compare_costs(
    account_id,
    None,
    "us-west-2",
    last_friday,
    this_friday,
    Some(group_by)
).await?;
*/
