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

#![cfg(feature = "integration-tests")]

use actix_web::{dev::Service as _, http::StatusCode, test, web, App, HttpMessage};
use mayyam::config::{Config, KafkaClusterConfig};
use mayyam::controllers::kafka::{
    get_kafka_acl_inventory_pillar_reports, get_kafka_admin_api_inventory_pillar_reports,
    get_kafka_broker_inventory_pillar_reports, get_kafka_cluster_inventory_pillar_reports,
    get_kafka_consumer_group_inventory_pillar_reports, get_kafka_consumer_inventory_pillar_reports,
    get_kafka_controller_quorum_inventory_pillar_reports,
    get_kafka_isr_health_inventory_pillar_reports, get_kafka_lag_inventory_pillar_reports,
    get_kafka_offset_inventory_pillar_reports, get_kafka_partition_inventory_pillar_reports,
    get_kafka_producer_inventory_pillar_reports, get_kafka_replica_inventory_pillar_reports,
    get_kafka_topic_inventory_pillar_reports,
};
use mayyam::middleware::auth::Claims;
use serde_json::Value;

#[tokio::test]
async fn kafka_cluster_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let mut config = Config::default();
    config.kafka.clusters = vec![KafkaClusterConfig {
        name: "orders".to_string(),
        bootstrap_servers: vec!["broker-1:9092".to_string()],
        sasl_username: None,
        sasl_password: None,
        sasl_mechanism: None,
        security_protocol: "PLAINTEXT".to_string(),
    }];

    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(config))
            .route(
                "/api/kafka/inventory/clusters/pillars",
                web::get().to(get_kafka_cluster_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/brokers/pillars",
                web::get().to(get_kafka_broker_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/controller-quorum/pillars",
                web::get().to(get_kafka_controller_quorum_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/topics/pillars",
                web::get().to(get_kafka_topic_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/partitions/pillars",
                web::get().to(get_kafka_partition_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/replicas/pillars",
                web::get().to(get_kafka_replica_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/isr-health/pillars",
                web::get().to(get_kafka_isr_health_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/consumer-groups/pillars",
                web::get().to(get_kafka_consumer_group_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/offsets/pillars",
                web::get().to(get_kafka_offset_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/lag/pillars",
                web::get().to(get_kafka_lag_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/producers/pillars",
                web::get().to(get_kafka_producer_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/consumers/pillars",
                web::get().to(get_kafka_consumer_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/admin-api/pillars",
                web::get().to(get_kafka_admin_api_inventory_pillar_reports),
            )
            .route(
                "/api/kafka/inventory/acls/pillars",
                web::get().to(get_kafka_acl_inventory_pillar_reports),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/clusters/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaCluster");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/clusters/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/clusters/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/brokers/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaBroker");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/brokers/pillars?pillar=resilience,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/brokers/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/controller-quorum/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaControllerQuorum");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/controller-quorum/pillars?pillar=cost,resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/controller-quorum/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/topics/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaTopic");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/topics/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/topics/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/partitions/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaPartition");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/partitions/pillars?pillar=resilience,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/partitions/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/replicas/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaReplica");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/replicas/pillars?pillar=cost,resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/replicas/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/isr-health/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaIsrHealth");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/isr-health/pillars?pillar=resilience,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/isr-health/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/consumer-groups/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaConsumerGroup");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/consumer-groups/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/consumer-groups/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/offsets/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaOffset");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/offsets/pillars?pillar=resilience,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/offsets/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/lag/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaLag");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/lag/pillars?pillar=cost,resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/lag/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/producers/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaProducer");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/producers/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/producers/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/consumers/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaConsumer");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/consumers/pillars?pillar=resilience,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/consumers/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/admin-api/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaAdminApi");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/admin-api/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/admin-api/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/acls/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KafkaAcl");
    assert_eq!(body["resources_evaluated"], 1);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");
    assert_eq!(reports[2]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/acls/pillars?pillar=resilience,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kafka/inventory/acls/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
