use actix_web::{test, web, App};
use actix_http::Request;
use serde_json::json;
use std::sync::Arc;

use crate::config::Config;
use crate::controllers::aws_account_controller::configure_aws_account_routes;
use crate::repositories::aws_account_repository::AwsAccountRepository;
use crate::services::aws_account_service::AwsAccountService;
use crate::test_utils::{setup_test_database, cleanup_test_database};

/// Integration tests for AWS Account API endpoints
#[cfg(test)]
mod aws_account_integration_tests {
    use super::*;

    /// Setup test application with all dependencies
    async fn setup_test_app() -> impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        // Setup test database
        let db = setup_test_database().await;

        // Create repository and service
        let repository = Arc::new(AwsAccountRepository::new(db.clone()));
        let service = Arc::new(AwsAccountService::new(repository.clone()));

        // Create test app
        test::init_service(
            App::new()
                .app_data(web::Data::new(service.clone()))
                .configure(configure_aws_account_routes)
        ).await
    }

    /// Test creating an AWS account via API
    #[actix_web::test]
    async fn test_create_aws_account_api() {
        let app = setup_test_app().await;

        let account_data = json!({
            "account_id": "123456789012",
            "account_name": "Test Account",
            "profile": "test-profile",
            "default_region": "us-east-1",
            "use_role": false,
            "access_key_id": "AKIAIOSFODNN7EXAMPLE",
            "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        });

        let req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&account_data)
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["account_id"], "123456789012");
        assert_eq!(body["account_name"], "Test Account");
    }

    /// Test getting all AWS accounts via API
    #[actix_web::test]
    async fn test_get_all_aws_accounts_api() {
        let app = setup_test_app().await;

        let req = test::TestRequest::get()
            .uri("/api/aws/accounts")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.is_empty()); // Should be empty initially
    }

    /// Test getting AWS account by ID via API
    #[actix_web::test]
    async fn test_get_aws_account_by_id_api() {
        let app = setup_test_app().await;

        // First create an account
        let account_data = json!({
            "account_id": "123456789012",
            "account_name": "Test Account",
            "profile": "test-profile",
            "default_region": "us-east-1",
            "use_role": false,
            "access_key_id": "AKIAIOSFODNN7EXAMPLE",
            "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        });

        let create_req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&account_data)
            .to_request();

        let create_resp = test::call_service(&app, create_req).await;
        let created_account: serde_json::Value = test::read_body_json(create_resp).await;
        let account_id = created_account["id"].as_str().unwrap();

        // Now get the account by ID
        let get_req = test::TestRequest::get()
            .uri(&format!("/api/aws/accounts/{}", account_id))
            .to_request();

        let get_resp = test::call_service(&app, get_req).await;

        assert_eq!(get_resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(get_resp).await;
        assert_eq!(body["account_id"], "123456789012");
        assert_eq!(body["account_name"], "Test Account");
    }

    /// Test updating AWS account via API
    #[actix_web::test]
    async fn test_update_aws_account_api() {
        let app = setup_test_app().await;

        // First create an account
        let account_data = json!({
            "account_id": "123456789012",
            "account_name": "Test Account",
            "profile": "test-profile",
            "default_region": "us-east-1",
            "use_role": false,
            "access_key_id": "AKIAIOSFODNN7EXAMPLE",
            "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        });

        let create_req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&account_data)
            .to_request();

        let create_resp = test::call_service(&app, create_req).await;
        let created_account: serde_json::Value = test::read_body_json(create_resp).await;
        let account_id = created_account["id"].as_str().unwrap();

        // Update the account
        let update_data = json!({
            "account_name": "Updated Test Account",
            "default_region": "us-west-2"
        });

        let update_req = test::TestRequest::put()
            .uri(&format!("/api/aws/accounts/{}", account_id))
            .set_json(&update_data)
            .to_request();

        let update_resp = test::call_service(&app, update_req).await;

        assert_eq!(update_resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(update_resp).await;
        assert_eq!(body["account_name"], "Updated Test Account");
        assert_eq!(body["default_region"], "us-west-2");
    }

    /// Test deleting AWS account via API
    #[actix_web::test]
    async fn test_delete_aws_account_api() {
        let app = setup_test_app().await;

        // First create an account
        let account_data = json!({
            "account_id": "123456789012",
            "account_name": "Test Account",
            "profile": "test-profile",
            "default_region": "us-east-1",
            "use_role": false,
            "access_key_id": "AKIAIOSFODNN7EXAMPLE",
            "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        });

        let create_req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&account_data)
            .to_request();

        let create_resp = test::call_service(&app, create_req).await;
        let created_account: serde_json::Value = test::read_body_json(create_resp).await;
        let account_id = created_account["id"].as_str().unwrap();

        // Delete the account
        let delete_req = test::TestRequest::delete()
            .uri(&format!("/api/aws/accounts/{}", account_id))
            .to_request();

        let delete_resp = test::call_service(&app, delete_req).await;

        assert_eq!(delete_resp.status(), 204);

        // Verify account is deleted
        let get_req = test::TestRequest::get()
            .uri(&format!("/api/aws/accounts/{}", account_id))
            .to_request();

        let get_resp = test::call_service(&app, get_req).await;
        assert_eq!(get_resp.status(), 404);
    }

    /// Test API error handling for invalid account ID
    #[actix_web::test]
    async fn test_get_nonexistent_account_api() {
        let app = setup_test_app().await;

        let req = test::TestRequest::get()
            .uri("/api/aws/accounts/550e8400-e29b-41d4-a716-446655440000")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["error"].is_string());
    }

    /// Test API validation for invalid data
    #[actix_web::test]
    async fn test_create_account_invalid_data_api() {
        let app = setup_test_app().await;

        let invalid_data = json!({
            "account_id": "invalid", // Invalid account ID format
            "account_name": "",
            "default_region": "invalid-region"
        });

        let req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&invalid_data)
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 400);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["error"].is_string());
    }
}

/// Integration tests for AWS Resource API endpoints
#[cfg(test)]
mod aws_resource_integration_tests {
    use super::*;
    use crate::controllers::aws_resource_controller::configure_aws_resource_routes;
    use crate::repositories::aws_resource_repository::AwsResourceRepository;
    use crate::services::aws_resource_service::AwsResourceService;

    /// Setup test application for AWS resources
    async fn setup_resource_test_app() -> impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        let db = setup_test_database().await;

        let repository = Arc::new(AwsResourceRepository::new(db.clone()));
        let service = Arc::new(AwsResourceService::new(repository.clone()));

        test::init_service(
            App::new()
                .app_data(web::Data::new(service.clone()))
                .configure(configure_aws_resource_routes)
        ).await
    }

    /// Test getting AWS resources by account
    #[actix_web::test]
    async fn test_get_aws_resources_by_account_api() {
        let app = setup_resource_test_app().await;

        let req = test::TestRequest::get()
            .uri("/api/aws/resources/account/123456789012")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.is_empty()); // Should be empty initially
    }

    /// Test getting AWS resources by type
    #[actix_web::test]
    async fn test_get_aws_resources_by_type_api() {
        let app = setup_resource_test_app().await;

        let req = test::TestRequest::get()
            .uri("/api/aws/resources/type/ec2")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.is_empty());
    }
}
