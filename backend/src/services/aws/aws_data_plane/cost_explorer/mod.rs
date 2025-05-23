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
    DateInterval,
    GroupDefinition,
    Granularity,
    Context,
    Expression,
    DimensionValues as AwsDimensionValues,
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
