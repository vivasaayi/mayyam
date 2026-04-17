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

use actix_web::web;

use crate::controllers::chaos;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/chaos")
            // Template endpoints
            .route("/templates", web::get().to(chaos::list_templates))
            .route("/templates", web::post().to(chaos::create_template))
            .route("/templates/{id}", web::get().to(chaos::get_template))
            .route("/templates/{id}", web::put().to(chaos::update_template))
            .route("/templates/{id}", web::delete().to(chaos::delete_template))
            .route(
                "/templates/{id}/create-experiment",
                web::post().to(chaos::create_experiment_from_template),
            )
            // Experiment endpoints
            .route("/experiments", web::get().to(chaos::list_experiments))
            .route(
                "/experiments/with-runs",
                web::get().to(chaos::list_experiments_with_runs),
            )
            .route("/experiments", web::post().to(chaos::create_experiment))
            .route("/experiments/{id}", web::get().to(chaos::get_experiment))
            .route("/experiments/{id}", web::put().to(chaos::update_experiment))
            .route(
                "/experiments/{id}",
                web::delete().to(chaos::delete_experiment),
            )
            // Execution endpoints
            .route(
                "/experiments/{id}/run",
                web::post().to(chaos::run_experiment),
            )
            .route(
                "/experiments/{id}/stop",
                web::post().to(chaos::stop_experiment),
            )
            .route(
                "/experiments/batch-run",
                web::post().to(chaos::batch_run_experiments),
            )
            // Run & Results endpoints
            .route(
                "/experiments/{id}/runs",
                web::get().to(chaos::list_experiment_runs),
            )
            .route(
                "/experiments/{id}/results",
                web::get().to(chaos::get_experiment_results),
            )
            .route("/runs/{id}", web::get().to(chaos::get_run))
            // Resource-centric endpoints
            .route(
                "/resources/{resource_id}/experiments",
                web::get().to(chaos::get_experiments_for_resource),
            )
            .route(
                "/resources/{resource_id}/history",
                web::get().to(chaos::get_resource_experiment_history),
            )
            // Audit logging endpoints
            .route("/audit/logs", web::get().to(chaos::list_audit_logs))
            .route(
                "/audit/experiments/{experiment_id}",
                web::get().to(chaos::get_experiment_audit_trail),
            )
            .route(
                "/audit/runs/{run_id}",
                web::get().to(chaos::get_run_audit_trail),
            )
            .route(
                "/audit/users/{user_id}",
                web::get().to(chaos::get_user_activity),
            )
            // Metrics endpoints
            .route("/metrics/stats", web::get().to(chaos::get_metrics_stats))
            .route(
                "/metrics/experiments/{experiment_id}",
                web::get().to(chaos::get_experiment_metrics),
            )
            .route(
                "/metrics/resource-types/{resource_type}",
                web::get().to(chaos::get_resource_type_metrics),
            ),
    );
}
