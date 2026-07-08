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

use actix_web::{web, HttpResponse, Result};
use std::sync::Arc;

use crate::controllers::cost_alerts::{CostAlertsController, CostAnomalyAlertQuery};

pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<CostAlertsController>) {
    cfg.service(
        web::scope("/api/v1/cost-alerts")
            .app_data(web::Data::from(controller))
            .route("/anomalies", web::get().to(anomaly_alerts)),
    );
}

async fn anomaly_alerts(
    controller: web::Data<CostAlertsController>,
    query: web::Query<CostAnomalyAlertQuery>,
) -> Result<HttpResponse> {
    CostAlertsController::anomaly_alerts(controller, query).await
}
