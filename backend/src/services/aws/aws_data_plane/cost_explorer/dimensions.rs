use serde_json::{json, Value};
use tracing::{debug, error};
use crate::{errors::AppError, models::aws_account::AwsAccountDto};
use super::base::AwsCostService;
use aws_sdk_costexplorer::types::{DateInterval, Context};

pub trait DimensionValues {
    async fn get_dimension_values(
        &self,
        account_id: &str,
        aws_account_dto: &AwsAccountDto,
        region: &str,
        dimension: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Value, AppError>;

    async fn get_available_dimensions(
        &self,
        account_id: &str,
        aws_account_dto: &AwsAccountDto,
        region: &str,
    ) -> Result<Vec<String>, AppError>;
}

impl DimensionValues for AwsCostService {
    async fn get_dimension_values(
        &self,
        account_id: &str,
        aws_account_dto: &AwsAccountDto,
        region: &str,
        dimension: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Value, AppError> {
        let client = self.create_client(aws_account_dto).await?;
        
        let time_period = DateInterval::builder()
            .start(start_date)
            .end(end_date)
            .build()
            .map_err(|e| AppError::ExternalService(format!("Failed to build time period: {}", e)))?;
        
        debug!("Fetching dimension values for {}", dimension);
        
        let response = client.get_dimension_values()
            .time_period(time_period)
            .dimension(aws_sdk_costexplorer::types::Dimension::from(dimension))
            .context(Context::CostAndUsage)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get dimension values: {}", e)))?;
        
        let mut result = json!({
            "dimension": dimension,
            "account_id": account_id,
            "values": []
        });

        for dimension_value in response.dimension_values() {
            if let Some(value) = dimension_value.value() {
                if let Some(values) = result["values"].as_array_mut() {
                    values.push(json!(value));
                }
            }
        }
        
        Ok(result)
    }

    async fn get_available_dimensions(
        &self,
        _account_id: &str,
        _aws_account_dto: &AwsAccountDto,
        _region: &str,
    ) -> Result<Vec<String>, AppError> {
        // AWS Cost Explorer supports these standard dimensions
        Ok(vec![
            "AZ".to_string(),
            "INSTANCE_TYPE".to_string(),
            "LINKED_ACCOUNT".to_string(),
            "OPERATION".to_string(),
            "PURCHASE_TYPE".to_string(),
            "REGION".to_string(),
            "SERVICE".to_string(),
            "USAGE_TYPE".to_string(),
            "PLATFORM".to_string(),
            "TENANCY".to_string(),
            "RECORD_TYPE".to_string(),
            "LEGAL_ENTITY_NAME".to_string(),
            "DEPLOYMENT_OPTION".to_string(),
            "DATABASE_ENGINE".to_string(),
            "CACHE_ENGINE".to_string(),
            "INSTANCE_TYPE_FAMILY".to_string(),
        ])
    }
}
