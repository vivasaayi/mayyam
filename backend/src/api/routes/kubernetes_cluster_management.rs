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


use actix_web::{web, Scope};
use std::sync::Arc;

use crate::controllers::kubernetes_cluster_management::KubernetesClusterManagementController; // Renamed controller

pub fn configure(
    cfg: &mut web::ServiceConfig,
    controller: Arc<KubernetesClusterManagementController>,
) {
    cfg.service(
        web::scope("/api/kubernetes-clusters") // Changed scope from /clusters
            .route("", web::get().to({ // Get all Kubernetes clusters
                let controller = controller.clone();
                move |query| {
                    let controller_clone = controller.clone();
                    // Method renamed in controller
                    async move { controller_clone.get_all_kubernetes_clusters(query).await }
                }
            }))
            .route("", web::post().to({ // Create Kubernetes cluster - path simplified
                let controller = controller.clone();
                move |claims, req_body| {
                    let controller_clone = controller.clone();
                    async move { controller_clone.create_kubernetes_cluster(claims, req_body).await }
                }
            }))
            .service(
                web::scope("/{cluster_id}") // Routes for specific cluster by ID
                    .route("", web::get().to({ // Get Kubernetes cluster by ID
                        let controller = controller.clone();
                        move |path| {
                            let controller_clone = controller.clone();
                            // Method renamed in controller
                            async move { controller_clone.get_kubernetes_cluster_by_id(path).await }
                        }
                    }))
                    .route("", web::put().to({ // Update Kubernetes cluster
                        let controller = controller.clone();
                        move |claims, path, req_body| {
                            let controller_clone = controller.clone();
                            async move { controller_clone.update_kubernetes_cluster(claims, path, req_body).await }
                        }
                    }))
                    .route("", web::delete().to({ // Delete Kubernetes cluster by ID
                        let controller = controller.clone();
                        move |claims, path| {
                            let controller_clone = controller.clone();
                            // Method renamed in controller
                            async move { controller_clone.delete_kubernetes_cluster(claims, path).await }
                        }
                    }))
            )
    );
}
