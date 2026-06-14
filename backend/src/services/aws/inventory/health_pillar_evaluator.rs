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

// Deterministic AWS Health account inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04978/04987/05014).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "HealthAccount";

pub const REASON_COST_NO_TAGS: &str = "HEALTH_COST_NO_TAGS";
pub const REASON_COST_RELEVANT_EVENTS: &str = "HEALTH_COST_RELEVANT_EVENTS";
pub const REASON_RES_OPEN_EVENTS: &str = "HEALTH_RES_OPEN_EVENTS";
pub const REASON_RES_UPCOMING_EVENTS: &str = "HEALTH_RES_UPCOMING_EVENTS";
pub const REASON_RES_AFFECTED_ENTITIES: &str = "HEALTH_RES_AFFECTED_ENTITIES";
pub const REASON_RES_ENTITY_COLLECTION_ERRORS: &str = "HEALTH_RES_ENTITY_COLLECTION_ERRORS";
pub const REASON_SEC_SECURITY_EVENTS: &str = "HEALTH_SEC_SECURITY_EVENTS";
pub const REASON_SEC_ENTITY_EVIDENCE_GAPS: &str = "HEALTH_SEC_ENTITY_EVIDENCE_GAPS";
pub const REASON_INV_STALE_DATA: &str = "HEALTH_INV_STALE_DATA";

pub fn evaluate_health_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != RESOURCE_TYPE {
            continue;
        }
        evaluated += 1;

        if let Some(stale) = check_stale(resource, pillar, REASON_INV_STALE_DATA, now) {
            stale_resources += 1;
            findings.push(stale);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn data_usize(resource_data: &Value, key: &str) -> usize {
    resource_data
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0)
}

fn finding(
    resource: &AwsResourceModel,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "AWS Health account {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    let cost_events = data_usize(&resource.resource_data, "cost_relevant_event_count");
    if cost_events > 0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_RELEVANT_EVENTS,
            Severity::Medium,
            format!(
                "AWS Health account {} has {} cost-relevant Health events",
                resource.resource_id, cost_events
            ),
            json!({
                "cost_relevant_event_count": cost_events,
                "events": resource.resource_data.get("events"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let open_events = data_usize(&resource.resource_data, "open_event_count")
        + data_usize(&resource.resource_data, "issue_event_count");
    if open_events > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_OPEN_EVENTS,
            Severity::High,
            format!(
                "AWS Health account {} has {} open or issue events",
                resource.resource_id, open_events
            ),
            json!({
                "open_event_count": resource.resource_data.get("open_event_count"),
                "issue_event_count": resource.resource_data.get("issue_event_count"),
                "events": resource.resource_data.get("events"),
            }),
        ));
    }

    let upcoming = data_usize(&resource.resource_data, "upcoming_event_count")
        + data_usize(&resource.resource_data, "scheduled_change_event_count");
    if upcoming > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_UPCOMING_EVENTS,
            Severity::Medium,
            format!(
                "AWS Health account {} has {} upcoming or scheduled-change events",
                resource.resource_id, upcoming
            ),
            json!({
                "upcoming_event_count": resource.resource_data.get("upcoming_event_count"),
                "scheduled_change_event_count": resource.resource_data.get("scheduled_change_event_count"),
                "events": resource.resource_data.get("events"),
            }),
        ));
    }

    let affected = data_usize(&resource.resource_data, "impaired_entity_count")
        + data_usize(&resource.resource_data, "pending_entity_count");
    if affected > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_AFFECTED_ENTITIES,
            Severity::High,
            format!(
                "AWS Health account {} has {} impaired or pending affected entities",
                resource.resource_id, affected
            ),
            json!({
                "impaired_entity_count": resource.resource_data.get("impaired_entity_count"),
                "pending_entity_count": resource.resource_data.get("pending_entity_count"),
                "affected_entities": resource.resource_data.get("affected_entities"),
            }),
        ));
    }

    let collection_errors = data_usize(
        &resource.resource_data,
        "affected_entity_collection_error_count",
    );
    if collection_errors > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_ENTITY_COLLECTION_ERRORS,
            Severity::Medium,
            format!(
                "AWS Health account {} had {} affected-entity collection errors",
                resource.resource_id, collection_errors
            ),
            json!({
                "affected_entity_collection_error_count": collection_errors,
                "event_count": resource.resource_data.get("event_count"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let security_events = data_usize(&resource.resource_data, "security_relevant_event_count");
    if security_events > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_SECURITY_EVENTS,
            Severity::High,
            format!(
                "AWS Health account {} has {} security-relevant Health events",
                resource.resource_id, security_events
            ),
            json!({
                "security_relevant_event_count": security_events,
                "events": resource.resource_data.get("events"),
            }),
        ));
    }

    if data_usize(&resource.resource_data, "event_count") > 0
        && data_usize(&resource.resource_data, "affected_entity_sample_count") == 0
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_ENTITY_EVIDENCE_GAPS,
            Severity::Medium,
            format!(
                "AWS Health account {} has events but no affected-entity samples",
                resource.resource_id
            ),
            json!({
                "event_count": resource.resource_data.get("event_count"),
                "affected_entity_sample_count": resource.resource_data.get("affected_entity_sample_count"),
                "affected_entity_collection_error_count": resource.resource_data.get("affected_entity_collection_error_count"),
            }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use serde_json::json;
    use uuid::Uuid;

    fn make_resource(resource_data: Value, tags: Value) -> AwsResourceModel {
        let now = Utc::now();
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: Some(Uuid::new_v4()),
            account_id: "123456789012".to_string(),
            profile: Some("test".to_string()),
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: "health:123456789012".to_string(),
            arn: "arn:aws:health:us-east-1:123456789012:account/123456789012".to_string(),
            name: Some("AWS Health".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_health_inventory_findings() {
        let now = Utc::now();
        let resource = make_resource(
            json!({
                "event_count": 2,
                "open_event_count": 1,
                "upcoming_event_count": 1,
                "issue_event_count": 1,
                "scheduled_change_event_count": 1,
                "cost_relevant_event_count": 1,
                "security_relevant_event_count": 1,
                "affected_entity_count": 2,
                "impaired_entity_count": 1,
                "pending_entity_count": 1,
                "affected_entity_collection_error_count": 1,
                "affected_entity_sample_count": 0,
                "events": [
                    {
                        "arn": "arn:aws:health:::event/EC2/example",
                        "service": "EC2",
                        "event_type_code": "AWS_EC2_SECURITY_NOTIFICATION",
                        "event_type_category": "issue",
                        "status_code": "open"
                    }
                ],
                "affected_entities": []
            }),
            json!({}),
        );

        let cost = evaluate_health_fleet(std::slice::from_ref(&resource), Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert_eq!(cost.stale_resources, 1);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_RELEVANT_EVENTS));

        let resilience =
            evaluate_health_fleet(std::slice::from_ref(&resource), Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_OPEN_EVENTS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_UPCOMING_EVENTS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_AFFECTED_ENTITIES));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_ENTITY_COLLECTION_ERRORS));

        let security =
            evaluate_health_fleet(std::slice::from_ref(&resource), Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_SECURITY_EVENTS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_ENTITY_EVIDENCE_GAPS));

        let tagged = make_resource(
            json!({
                "event_count": 0,
                "cost_relevant_event_count": 0,
                "security_relevant_event_count": 0,
                "affected_entity_sample_count": 0
            }),
            json!({ "CostCenter": "platform" }),
        );
        let healthy = evaluate_health_fleet(&[tagged], Pillar::Cost, now);
        assert!(healthy
            .findings
            .iter()
            .all(|finding| finding.reason_code != REASON_COST_NO_TAGS));
    }
}
