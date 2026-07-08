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

use crate::models::aws_resource::AwsResourceType;
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::services::alerting::cloud_quota_exhaustion_alert::{
    build_cloud_quota_exhaustion_alert_bundle, stale_after_hours, RESOURCE_TYPE,
};

pub struct CloudAlertsController {
    aws_resource_repo: Arc<AwsResourceRepository>,
}

impl CloudAlertsController {
    pub fn new(aws_resource_repo: Arc<AwsResourceRepository>) -> Self {
        Self { aws_resource_repo }
    }

    pub async fn quota_exhaustion_alerts(
        controller: web::Data<CloudAlertsController>,
        query: web::Query<CloudQuotaAlertQuery>,
    ) -> ActixResult<HttpResponse> {
        let route53_resources = controller
            .aws_resource_repo
            .find_by_account_and_type(
                &query.account_id,
                &AwsResourceType::Route53HostedZone.to_string(),
            )
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
        let eventbridge_resources = controller
            .aws_resource_repo
            .find_by_account_and_type(
                &query.account_id,
                &AwsResourceType::EventBridgeRule.to_string(),
            )
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
        let lambda_resources = controller
            .aws_resource_repo
            .find_by_account_and_type(
                &query.account_id,
                &AwsResourceType::LambdaFunction.to_string(),
            )
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;

        let now = chrono::Utc::now();
        let bundle = build_cloud_quota_exhaustion_alert_bundle(
            &route53_resources,
            &eventbridge_resources,
            &lambda_resources,
        );

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": RESOURCE_TYPE,
            "evaluated_at": now,
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
pub struct CloudQuotaAlertQuery {
    pub account_id: String,
}
