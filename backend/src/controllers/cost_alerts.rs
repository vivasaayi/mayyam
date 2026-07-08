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

use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::Deserialize;
use std::sync::Arc;

use crate::repositories::cost_analytics::CostAnalyticsRepository;
use crate::services::alerting::cost_anomaly_alert::{
    build_cost_anomaly_alert_bundle, evaluated_at, stale_after_hours, RESOURCE_TYPE,
};

pub struct CostAlertsController {
    cost_analytics_repo: Arc<CostAnalyticsRepository>,
}

impl CostAlertsController {
    pub fn new(cost_analytics_repo: Arc<CostAnalyticsRepository>) -> Self {
        Self {
            cost_analytics_repo,
        }
    }

    pub async fn anomaly_alerts(
        controller: web::Data<CostAlertsController>,
        query: web::Query<CostAnomalyAlertQuery>,
    ) -> ActixResult<HttpResponse> {
        let anomalies = controller
            .cost_analytics_repo
            .get_cost_anomalies_by_account(
                &query.account_id,
                query.severity.clone(),
                query.status.clone(),
            )
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;

        let mut latest_insights = Vec::with_capacity(anomalies.len());
        for anomaly in &anomalies {
            let insight = controller
                .cost_analytics_repo
                .get_cost_insights_by_anomaly(anomaly.id)
                .await
                .map_err(actix_web::error::ErrorInternalServerError)?
                .into_iter()
                .next();
            latest_insights.push(insight);
        }

        let bundle = build_cost_anomaly_alert_bundle(&anomalies, &latest_insights);

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": RESOURCE_TYPE,
            "evaluated_at": evaluated_at(),
            "stale_after_hours": stale_after_hours(),
            "resources_evaluated": bundle.resources_evaluated,
            "active_alerts": bundle.active_alerts,
            "insufficient_data_alerts": bundle.insufficient_data_alerts,
            "alerts": bundle.alerts,
            "notification_workflow": bundle.notification_workflow,
            "ai_triage_workflow": bundle.ai_triage_workflow,
            "agentic_investigation_workflow": bundle.agentic_investigation_workflow,
            "automation_workflow": bundle.automation_workflow,
            "health_workflow": bundle.health_workflow,
            "reporting": bundle.reporting,
        })))
    }
}

#[derive(Debug, Deserialize)]
pub struct CostAnomalyAlertQuery {
    pub account_id: String,
    pub severity: Option<String>,
    pub status: Option<String>,
}
