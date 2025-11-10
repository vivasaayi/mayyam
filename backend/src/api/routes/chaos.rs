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


use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = web::scope("/api/chaos")
        .route("/experiments", web::get().to(list_experiments))
        .route("/experiments", web::post().to(create_experiment))
        .route("/experiments/{id}", web::get().to(get_experiment))
        .route("/experiments/{id}", web::delete().to(delete_experiment))
        .route("/experiments/{id}/start", web::post().to(start_experiment))
        .route("/experiments/{id}/stop", web::post().to(stop_experiment))
        .route(
            "/experiments/{id}/results",
            web::get().to(get_experiment_results),
        );

    cfg.service(scope);
}

async fn list_experiments() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "This endpoint will list all chaos experiments",
        "experiments": []
    }))
}

async fn create_experiment() -> HttpResponse {
    HttpResponse::Created().json(serde_json::json!({
        "message": "This endpoint will create a new chaos experiment",
        "id": "exp-12345",
        "success": true
    }))
}

async fn get_experiment(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will fetch chaos experiment with ID: {}", id),
        "id": id,
        "name": "Example experiment",
        "type": "network_delay",
        "target": "service-a",
        "parameters": {
            "duration": 60,
            "delay": "100ms"
        },
        "status": "ready"
    }))
}

async fn delete_experiment(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will delete chaos experiment with ID: {}", id),
        "id": id,
        "success": true
    }))
}

async fn start_experiment(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will start chaos experiment with ID: {}", id),
        "id": id,
        "status": "running",
        "started_at": "2025-05-14T12:00:00Z"
    }))
}

async fn stop_experiment(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will stop chaos experiment with ID: {}", id),
        "id": id,
        "status": "stopped",
        "stopped_at": "2025-05-14T12:05:00Z"
    }))
}

async fn get_experiment_results(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "message": format!("This endpoint will get results for chaos experiment with ID: {}", id),
        "id": id,
        "status": "completed",
        "metrics": {
            "before": {
                "latency_p50": 10,
                "latency_p99": 50,
                "error_rate": 0.001
            },
            "during": {
                "latency_p50": 110,
                "latency_p99": 500,
                "error_rate": 0.05
            },
            "after": {
                "latency_p50": 12,
                "latency_p99": 55,
                "error_rate": 0.002
            }
        }
    }))
}
