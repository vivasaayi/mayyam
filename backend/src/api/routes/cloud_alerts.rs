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

use crate::controllers::cloud_alerts::{CloudAlertsController, CloudQuotaAlertQuery};

pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<CloudAlertsController>) {
    cfg.service(
        web::scope("/api/v1/cloud-alerts")
            .app_data(web::Data::from(controller))
            .route("/quota-exhaustion", web::get().to(quota_exhaustion_alerts)),
    );
}

async fn quota_exhaustion_alerts(
    controller: web::Data<CloudAlertsController>,
    query: web::Query<CloudQuotaAlertQuery>,
) -> Result<HttpResponse> {
    CloudAlertsController::quota_exhaustion_alerts(controller, query).await
}
