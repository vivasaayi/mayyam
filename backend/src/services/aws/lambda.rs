use std::sync::Arc;
use serde_json::json;
use tracing::info;

use crate::errors::AppError;
use crate::models::aws_resource;
use super::AwsService;
use crate::models::aws_resource::{AwsResourceType, AwsResourceDto, Model as AwsResourceModel};
use crate::models::aws_auth::AccountAuthInfo;
use super::types::*;
use super::client_factory::AwsClientFactory;

pub struct LambdaControlPlane {
    aws_service: Arc<AwsService>,
}

impl LambdaControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_functions(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<aws_resource::Model>, AppError> {
        self.sync_functions_with_auth(account_id, profile, region, None).await
    }

    pub async fn sync_functions_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<aws_resource::Model>, AppError> {
        let client = self.aws_service.create_lambda_client_with_auth(profile, region, account_auth).await?;
        self.sync_functions_with_client(account_id, profile, region, client).await
    }

    pub async fn sync_functions_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, _client: aws_sdk_lambda::Client) -> Result<Vec<aws_resource::Model>, AppError> {
        let repo = &self.aws_service.aws_resource_repo;
        
        let mut functions = Vec::new();
        
        let function_data = json!({
            "function_name": "sample-lambda-function",
            "function_arn": format!("arn:aws:lambda:{}:{}:function:sample-lambda-function", region, account_id),
            "runtime": "nodejs18.x",
            "role": format!("arn:aws:iam::{}:role/lambda-role", account_id),
            "handler": "index.handler",
            "code_size": 1024,
            "description": "Sample Lambda function for development",
            "timeout": 30,
            "memory_size": 128,
            "last_modified": "2023-07-01T12:00:00.000+0000",
            "code_sha256": "abcdef1234567890",
            "environment": {
                "variables": {
                    "ENV": "development",
                    "LOG_LEVEL": "info"
                }
            },
            "version": "$LATEST",
            "architectures": ["x86_64"]
        });
        
        let function = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::LambdaFunction.to_string(),
            resource_id: "sample-lambda-function".to_string(),
            arn: format!("arn:aws:lambda:{}:{}:function:sample-lambda-function", region, account_id),
            name: Some("Sample Lambda Function".to_string()),
            tags: json!({"Name": "Sample Lambda Function", "Environment": "Development"}),
            resource_data: function_data,
        };
        
        let saved_function = match repo.find_by_arn(&function.arn).await? {
            Some(existing) => repo.update(existing.id, &function).await?,
            None => repo.create(&function).await?,
        };
        functions.push(saved_function);
        
        Ok(functions)
    }
}

pub struct LambdaDataPlane {
    aws_service: Arc<AwsService>,
}

impl LambdaDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn invoke_function(&self, profile: Option<&str>, region: &str, request: &LambdaInvokeRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_lambda_client(profile, region).await?;
        
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
