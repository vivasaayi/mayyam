use actix_web::{test, web, App, http::StatusCode};
use std::sync::Arc;
use rstest::*;
use serial_test::serial;

use crate::controllers::aws_account;
use crate::services::aws_account::AwsAccountService;
use crate::repositories::aws_account::AwsAccountRepository;
use crate::models::aws_account::{AwsAccountCreateDto, AwsAccountDto};
use crate::services::aws::AwsControlPlane;

use crate::test_utils::{TestDb, get_test_db};

/// Test AWS Account Controller
#[cfg(test)]
mod aws_account_controller_tests {
    use super::*;

    /// Helper function to create test app
    async fn create_test_app() -> impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        // Create mock AWS control plane
        let aws_control_plane = Arc::new(AwsControlPlane::new(
            Arc::new(crate::services::aws::AwsService::new(
                Arc::new(crate::repositories::aws_resource::AwsResourceRepository::new(
                    Arc::new(test_db.conn().clone()),
                    crate::config::Config::default(),
                )),
                crate::config::Config::default(),
            ))
        ));

        let service = Arc::new(AwsAccountService::new(repo, aws_control_plane));

        test::init_service(
            App::new()
                .app_data(web::Data::new(service))
                .route("/api/aws/accounts", web::post().to(aws_account::create_account))
                .route("/api/aws/accounts", web::get().to(aws_account::list_accounts))
                .route("/api/aws/accounts/{id}", web::get().to(aws_account::get_account))
                .route("/api/aws/accounts/{id}", web::put().to(aws_account::update_account))
                .route("/api/aws/accounts/{id}", web::delete().to(aws_account::delete_account))
        ).await
    }

    /// Test creating an account via HTTP
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_create_account_endpoint() {
        let mut app = create_test_app().await;

        let create_dto = factories::fake_aws_account();

        let req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&create_dto)
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let result: AwsAccountDto = test::read_body_json(resp).await;
        assert_eq!(result.account_id, create_dto.account_id);
        assert_eq!(result.account_name, create_dto.account_name);
    }

    /// Test getting an account via HTTP
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_get_account_endpoint() {
        let mut app = create_test_app().await;

        // First create an account
        let create_dto = factories::fake_aws_account();
        let create_req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&create_dto)
            .to_request();

        let create_resp = test::call_service(&mut app, create_req).await;
        let created: AwsAccountDto = test::read_body_json(create_resp).await;

        // Now get it
        let get_req = test::TestRequest::get()
            .uri(&format!("/api/aws/accounts/{}", created.id))
            .to_request();

        let get_resp = test::call_service(&mut app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::OK);

        let retrieved: AwsAccountDto = test::read_body_json(get_resp).await;
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.account_id, created.account_id);
    }

    /// Test getting non-existent account
    #[rstest]
    #[tokio::test]
    async fn test_get_nonexistent_account_endpoint() {
        let mut app = create_test_app().await;

        let non_existent_id = uuid::Uuid::new_v4();
        let req = test::TestRequest::get()
            .uri(&format!("/api/aws/accounts/{}", non_existent_id))
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// Test listing accounts via HTTP
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_list_accounts_endpoint() {
        let mut app = create_test_app().await;

        // Create a couple of accounts first
        let account1 = factories::fake_aws_account();
        let account2 = factories::fake_aws_account();

        let create_req1 = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&account1)
            .to_request();
        test::call_service(&mut app, create_req1).await;

        let create_req2 = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&account2)
            .to_request();
        test::call_service(&mut app, create_req2).await;

        // Now list them
        let list_req = test::TestRequest::get()
            .uri("/api/aws/accounts")
            .to_request();

        let list_resp = test::call_service(&mut app, list_req).await;
        assert_eq!(list_resp.status(), StatusCode::OK);

        let accounts: Vec<AwsAccountDto> = test::read_body_json(list_resp).await;
        assert!(accounts.len() >= 2);
    }

    /// Test updating an account via HTTP
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_update_account_endpoint() {
        let mut app = create_test_app().await;

        // Create account
        let create_dto = factories::fake_aws_account();
        let create_req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&create_dto)
            .to_request();

        let create_resp = test::call_service(&mut app, create_req).await;
        let created: AwsAccountDto = test::read_body_json(create_resp).await;

        // Update account
        let update_dto = crate::models::aws_account::AwsAccountUpdateDto {
            account_name: Some("Updated via HTTP".to_string()),
            default_region: Some("us-west-2".to_string()),
            profile: Some("http-updated".to_string()),
            use_role: Some(false),
            role_arn: None,
            external_id: None,
            access_key_id: Some("HTTP_KEY".to_string()),
            secret_access_key: Some("HTTP_SECRET".to_string()),
        };

        let update_req = test::TestRequest::put()
            .uri(&format!("/api/aws/accounts/{}", created.id))
            .set_json(&update_dto)
            .to_request();

        let update_resp = test::call_service(&mut app, update_req).await;
        assert_eq!(update_resp.status(), StatusCode::OK);

        let updated: AwsAccountDto = test::read_body_json(update_resp).await;
        assert_eq!(updated.account_name, "Updated via HTTP");
        assert_eq!(updated.default_region, "us-west-2");
    }

    /// Test deleting an account via HTTP
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_delete_account_endpoint() {
        let mut app = create_test_app().await;

        // Create account
        let create_dto = factories::fake_aws_account();
        let create_req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&create_dto)
            .to_request();

        let create_resp = test::call_service(&mut app, create_req).await;
        let created: AwsAccountDto = test::read_body_json(create_resp).await;

        // Delete account
        let delete_req = test::TestRequest::delete()
            .uri(&format!("/api/aws/accounts/{}", created.id))
            .to_request();

        let delete_resp = test::call_service(&mut app, delete_req).await;
        assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

        // Verify it's deleted
        let get_req = test::TestRequest::get()
            .uri(&format!("/api/aws/accounts/{}", created.id))
            .to_request();

        let get_resp = test::call_service(&mut app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
    }

    /// Test invalid JSON payload
    #[rstest]
    #[tokio::test]
    async fn test_invalid_json_payload() {
        let mut app = create_test_app().await;

        let req = test::TestRequest::post()
            .uri("/api/aws/accounts")
            .set_json(&serde_json::json!({"invalid": "payload"}))
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        // Should return 400 Bad Request for invalid payload
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// Test malformed UUID in path
    #[rstest]
    #[tokio::test]
    async fn test_malformed_uuid() {
        let mut app = create_test_app().await;

        let req = test::TestRequest::get()
            .uri("/api/aws/accounts/not-a-uuid")
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
