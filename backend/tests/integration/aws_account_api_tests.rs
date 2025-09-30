#![cfg(feature = "integration-tests")]

use actix_web::{test, web, App};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use mayyam::config;
use mayyam::controllers::aws_account;
use mayyam::middleware::auth::{generate_token, AuthMiddleware};
use mayyam::models::aws_account::{AwsAccountCreateDto, AwsAccountDto};
use mayyam::repositories::aws_account::AwsAccountRepository;
use mayyam::repositories::aws_resource::AwsResourceRepository;
use mayyam::repositories::sync_run::SyncRunRepository;
use mayyam::services::aws::{AwsControlPlane, AwsService};
use mayyam::services::aws_account::AwsAccountService;
use mayyam::utils::database;

#[actix_web::test]
async fn aws_account_crud_flow() {
    let mut config = config::load_config().expect("load config");

    if config.database.postgres.is_empty() {
        if let Ok(host) = std::env::var("POSTGRES_HOST") {
            let user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "postgres".into());
            let pass = std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "postgres".into());
            let dbname = std::env::var("POSTGRES_DB").unwrap_or_else(|_| "mayyam_test".into());
            let port: u16 = std::env::var("POSTGRES_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5432);
            config.database.postgres.push(config::PostgresConfig {
                name: "test".into(),
                host,
                port,
                username: user,
                password: pass,
                database: dbname,
                ssl_mode: None,
            });
        }
    }

    let db: Arc<DatabaseConnection> = Arc::new(database::connect(&config).await.unwrap());

    let repo = Arc::new(AwsAccountRepository::new(db.clone()));

    let aws_service = Arc::new(AwsService::new(
        Arc::new(AwsResourceRepository::new(db.clone(), config.clone())),
        config.clone(),
    ));
    let aws_control_plane = Arc::new(AwsControlPlane::new(aws_service));
    let sync_repo = Arc::new(SyncRunRepository::new(db.clone()));

    let service = Arc::new(AwsAccountService::new(
        repo.clone(),
        aws_control_plane.clone(),
        sync_repo,
    ));

    let mut app = test::init_service(
        App::new()
            .wrap(AuthMiddleware::new(&config))
            .app_data(web::Data::new(service.clone()))
            .route("/api/aws/accounts", web::post().to(aws_account::create_account))
            .route("/api/aws/accounts", web::get().to(aws_account::list_accounts))
            .route("/api/aws/accounts/{id}", web::get().to(aws_account::get_account))
            .route(
                "/api/aws/accounts/{id}",
                web::put().to(aws_account::update_account),
            )
            .route(
                "/api/aws/accounts/{id}",
                web::delete().to(aws_account::delete_account),
            ),
    )
    .await;

    let token = generate_token(
        "test-user",
        "admin",
        Some("admin@example.com"),
        vec!["admin".to_string()],
        &config,
    )
    .expect("token");

    let create_dto = AwsAccountCreateDto {
        account_id: "123456789012".to_string(),
        account_name: "Test AWS Account".to_string(),
        profile: Some("default".to_string()),
        default_region: "us-east-1".to_string(),
        use_role: false,
        role_arn: None,
        external_id: None,
        access_key_id: Some(
            std::env::var("AWS_ACCESS_KEY_ID")
                .unwrap_or_else(|_| "AKIAIOSFODNN7EXAMPLE".to_string()),
        ),
        secret_access_key: Some(
            std::env::var("AWS_SECRET_ACCESS_KEY")
                .unwrap_or_else(|_| "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string()),
        ),
    };

    let req = test::TestRequest::post()
        .uri("/api/aws/accounts")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&create_dto)
        .to_request();
    let resp: AwsAccountDto = test::call_and_read_body_json(&mut app, req).await;

    assert_eq!(resp.account_id, "123456789012");
    assert_eq!(resp.account_name, "Test AWS Account");
    assert_eq!(resp.default_region, "us-east-1");
    assert!(resp.has_access_key);

    let account_id = resp.id;

    let req = test::TestRequest::get()
        .uri(&format!("/api/aws/accounts/{}", account_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp: AwsAccountDto = test::call_and_read_body_json(&mut app, req).await;

    assert_eq!(resp.id, account_id);
    assert_eq!(resp.account_id, "123456789012");

    let req = test::TestRequest::get()
        .uri("/api/aws/accounts")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp: Vec<AwsAccountDto> = test::call_and_read_body_json(&mut app, req).await;

    assert!(!resp.is_empty());
    assert!(resp.iter().any(|a| a.id == account_id));

    let update_dto = AwsAccountCreateDto {
        account_id: "123456789012".to_string(),
        account_name: "Updated AWS Account".to_string(),
        profile: Some("updated-profile".to_string()),
        default_region: "us-east-1".to_string(),
        use_role: false,
        role_arn: None,
        external_id: None,
        access_key_id: Some("AKIAIOSFODNN7EXAMPLE".to_string()),
        secret_access_key: Some("".to_string()),
    };

    let req = test::TestRequest::put()
        .uri(&format!("/api/aws/accounts/{}", account_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&update_dto)
        .to_request();
    let resp: AwsAccountDto = test::call_and_read_body_json(&mut app, req).await;

    assert_eq!(resp.id, account_id);
    assert_eq!(resp.account_name, "Updated AWS Account");

    let req = test::TestRequest::delete()
        .uri(&format!("/api/aws/accounts/{}", account_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&mut app, req).await;

    assert!(resp.status().is_success());

    let req = test::TestRequest::get()
        .uri(&format!("/api/aws/accounts/{}", account_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&mut app, req).await;

    assert_eq!(resp.status().as_u16(), 404);
}
