use actix_web::{test, App};

#[actix_rt::test]
async fn chat_stream_empty_messages_returns_400() {
    // Build the app using the real route configure function inside the crate
    let app = test::init_service(App::new().configure(|cfg| {
        crate::api::routes::ai::configure(cfg);
    }))
    .await;

    let payload = serde_json::json!({
        "messages": [],
        "model": null,
        "temperature": 1.0
    });

    let req = test::TestRequest::post()
        .uri("/api/ai/chat/stream")
        .set_json(&payload)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}
