use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::services::aws::aws_types::lambda::LambdaInvokeRequest;
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;
use std::sync::Arc;
use tracing::info;
use uuid;

pub struct LambdaDataPlane {
    aws_service: Arc<AwsService>,
}

impl LambdaDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn invoke_function(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &LambdaInvokeRequest,
    ) -> Result<serde_json::Value, AppError> {
        let client = self
            .aws_service
            .create_lambda_client(aws_account_dto)
            .await?;

        info!("Invoking Lambda function {}", request.function_name);

        // Mock implementation
        let response = json!({
            "status_code": 200,
            "function_error": null,
            "log_result": "U1RBUlQgUmVxdWVzdElkOiA0NWVjMTAwNy1iMDhiLTExZTctYWI1NS04YzE3M2YxMjNlODAgVmVyc2lvbjogJExBVEVTVAoyMDIzLTA3LTAxVDEyOjAwOjAwLjAwMFoJNDVlYzEwMDctYjA4Yi0xMWU3LWFiNTUtOGMxNzNmMTIzZTgwCUlORk8JU3VjY2Vzc2Z1bGx5IHByb2Nlc3NlZCByZXF1ZXN0CkVORCBSZXF1ZXN0SWQ6IDQ1ZWMxMDA3LWIwOGItMTFlNy1hYjU1LThjMTczZjEyM2U4MApSRVBPUlQgUmVxdWVzdElkOiA0NWVjMTAwNy1iMDhiLTExZTctYWI1NS04YzE3M2YxMjNlODAJRHVyYXRpb246IDEyMy40NSBtcwlCaWxsZWQgRHVyYXRpb246IDEyNCBtcwlNZW1vcnkgU2l6ZTogMTI4IE1CCU1heCBNZW1vcnkgVXNlZDogNjQgTUI=",
            "executed_version": "$LATEST",
            "payload": {
                "status": "success",
                "message": "Function executed successfully",
                "timestamp": "2023-07-01T12:00:00Z"
            }
        });

        Ok(response)
    }
}
