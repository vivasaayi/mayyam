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

// Deterministic ACM certificate inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-03718/03727/03754).
//
// Evaluates fields persisted by acm_control_plane: certificate_arn,
// domain_name, status, certificate_type, in_use_by, not_after,
// renewal_eligibility, key_algorithm, subject_alternative_names.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "ACM_COST_NO_TAGS";
pub const REASON_COST_UNUSED_CERTIFICATE: &str = "ACM_COST_UNUSED_CERTIFICATE";
pub const REASON_SEC_CERT_EXPIRED: &str = "ACM_SEC_CERT_EXPIRED";
pub const REASON_SEC_CERT_NOT_VALIDATED: &str = "ACM_SEC_CERT_NOT_VALIDATED";
pub const REASON_SEC_EXPIRY_DATA_NOT_COLLECTED: &str = "ACM_SEC_EXPIRY_DATA_NOT_COLLECTED";
pub const REASON_RES_EXPIRING_SOON: &str = "ACM_RES_EXPIRING_SOON";
pub const REASON_RES_EXPIRY_DATA_UNPARSEABLE: &str = "ACM_RES_EXPIRY_DATA_UNPARSEABLE";
pub const REASON_RES_RENEWAL_INELIGIBLE: &str = "ACM_RES_RENEWAL_INELIGIBLE";
pub const REASON_INV_STALE_DATA: &str = "ACM_INV_STALE_DATA";

/// Certificates expiring within this many days of `now` are flagged.
pub const EXPIRY_WARNING_DAYS: i64 = 30;

/// Statuses meaning the certificate never completed validation.
const UNVALIDATED_STATUSES: &[&str] = &["PENDING_VALIDATION", "VALIDATION_TIMED_OUT", "FAILED"];

/// Evaluate every ACM certificate in the fleet for one pillar.
/// Rows with a different `resource_type` are skipped and not counted.
pub fn evaluate_acm_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != "AcmCertificate" {
            continue;
        }
        evaluated += 1;
        if let Some(stale) = check_stale(resource, pillar, REASON_INV_STALE_DATA, now) {
            stale_resources += 1;
            findings.push(stale);
        }
        match pillar {
            Pillar::Cost => evaluate_cost(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, now, &mut findings),
            // Pillars without checks for this service yet produce no findings.
            _ => {}
        }
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score,
        findings,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let tags_empty = resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true);
    if tags_empty {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Certificate {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // Only flag unused when in_use_by was actually collected (key present)
    // and the certificate is issued; an unused issued certificate is
    // hygiene/sprawl, not direct spend.
    let status = data_str(&resource.resource_data, "status");
    let in_use_by = resource
        .resource_data
        .get("in_use_by")
        .and_then(|v| v.as_array());
    if let Some(in_use_by) = in_use_by {
        if in_use_by.is_empty() && status.as_deref() == Some("ISSUED") {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_UNUSED_CERTIFICATE.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Certificate {} is issued but not in use by any AWS resource; unused certificates accumulate as inventory sprawl",
                    resource.resource_id
                ),
                evidence: json!({ "status": "ISSUED", "in_use_by": [] }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let status = data_str(&resource.resource_data, "status");

    if status.as_deref() == Some("EXPIRED") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_CERT_EXPIRED.to_string(),
            severity: Severity::High,
            message: format!(
                "Certificate {} is expired; any endpoint still serving it presents an invalid certificate to clients",
                resource.resource_id
            ),
            evidence: json!({ "status": "EXPIRED" }),
        });
    }

    if let Some(status_value) = status.as_deref() {
        if UNVALIDATED_STATUSES.contains(&status_value) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_CERT_NOT_VALIDATED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Certificate {} has status {}; domain validation never completed and the certificate cannot be used",
                    resource.resource_id, status_value
                ),
                evidence: json!({ "status": status_value }),
            });
        }
    }

    let not_after = data_str(&resource.resource_data, "not_after");
    if status.as_deref() == Some("ISSUED") && not_after.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_EXPIRY_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Expiry date for issued certificate {} is not collected yet; expiry posture cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "status": "ISSUED", "not_after_collected": false }),
        });
    }
}

fn evaluate_resilience(
    resource: &AwsResourceModel,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
    if let Some(not_after_raw) = data_str(&resource.resource_data, "not_after") {
        match DateTime::parse_from_rfc3339(&not_after_raw) {
            Ok(not_after) => {
                let days_until_expiry = (not_after.with_timezone(&Utc) - now).num_days();
                if days_until_expiry <= EXPIRY_WARNING_DAYS {
                    findings.push(InventoryFinding {
                        resource_id: resource.resource_id.clone(),
                        arn: resource.arn.clone(),
                        pillar: Pillar::Resilience,
                        reason_code: REASON_RES_EXPIRING_SOON.to_string(),
                        severity: Severity::High,
                        message: format!(
                            "Certificate {} expires in {} days (threshold {} days); endpoints relying on it lose TLS when it lapses",
                            resource.resource_id, days_until_expiry, EXPIRY_WARNING_DAYS
                        ),
                        evidence: json!({
                            "not_after": not_after_raw,
                            "days_until_expiry": days_until_expiry,
                            "warning_threshold_days": EXPIRY_WARNING_DAYS,
                        }),
                    });
                }
            }
            Err(_) => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_EXPIRY_DATA_UNPARSEABLE.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Expiry date for certificate {} could not be parsed; expiry risk cannot be assessed",
                        resource.resource_id
                    ),
                    evidence: json!({ "not_after": not_after_raw }),
                });
            }
        }
    }

    let renewal_eligibility = data_str(&resource.resource_data, "renewal_eligibility");
    let certificate_type = data_str(&resource.resource_data, "certificate_type");
    if renewal_eligibility.as_deref() == Some("INELIGIBLE")
        && certificate_type.as_deref() == Some("IMPORTED")
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_RENEWAL_INELIGIBLE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Imported certificate {} is ineligible for managed renewal; manual rotation is required before expiry",
                resource.resource_id
            ),
            evidence: json!({
                "certificate_type": "IMPORTED",
                "renewal_eligibility": "INELIGIBLE",
            }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::Value;
    use uuid::Uuid;

    fn fixture(
        resource_id: &str,
        tags: Value,
        resource_data: Value,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: "AcmCertificate".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:acm:us-east-1:123456789012:certificate/{}",
                resource_id
            ),
            name: Some("example.com".to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_data() -> Value {
        json!({
            "certificate_arn": "arn:aws:acm:us-east-1:123456789012:certificate/cert-ok",
            "domain_name": "example.com",
            "status": "ISSUED",
            "certificate_type": "AMAZON_ISSUED",
            "in_use_by": ["arn:aws:elasticloadbalancing:us-east-1:123456789012:loadbalancer/app/web/abc"],
            "not_after": "2027-09-01T00:00:00Z",
            "renewal_eligibility": "ELIGIBLE",
            "key_algorithm": "RSA-2048",
            "subject_alternative_names": ["example.com", "www.example.com"],
        })
    }

    fn codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_untagged_certificate() {
        let r = fixture("cert-untagged", json!({}), healthy_data(), now());
        let report = evaluate_acm_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_unused_issued_certificate() {
        let mut data = healthy_data();
        data["in_use_by"] = json!([]);
        let r = fixture("cert-unused", json!({"team": "web"}), data, now());
        let report = evaluate_acm_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_UNUSED_CERTIFICATE]);
    }

    #[test]
    fn security_flags_expired_certificate() {
        let mut data = healthy_data();
        data["status"] = json!("EXPIRED");
        data["not_after"] = json!("2026-05-01T00:00:00Z");
        let r = fixture("cert-expired", json!({"team": "web"}), data, now());
        let report = evaluate_acm_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_CERT_EXPIRED]);
    }

    #[test]
    fn security_flags_unvalidated_certificate() {
        for status in ["PENDING_VALIDATION", "VALIDATION_TIMED_OUT", "FAILED"] {
            let data = json!({
                "certificate_arn": "arn:aws:acm:us-east-1:123456789012:certificate/cert-pending",
                "domain_name": "pending.example.com",
                "status": status,
                "certificate_type": "AMAZON_ISSUED",
                "in_use_by": [],
                "subject_alternative_names": ["pending.example.com"],
            });
            let r = fixture("cert-pending", json!({"team": "web"}), data, now());
            let report = evaluate_acm_fleet(&[r], Pillar::Security, now());
            assert_eq!(
                codes(&report),
                vec![REASON_SEC_CERT_NOT_VALIDATED],
                "expected not-validated finding for status {}",
                status
            );
        }
    }

    #[test]
    fn security_reports_gap_when_expiry_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("not_after");
        let r = fixture("cert-no-expiry", json!({"team": "web"}), data, now());
        let report = evaluate_acm_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_EXPIRY_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_certificate_expiring_within_30_days() {
        let mut data = healthy_data();
        data["not_after"] = json!("2026-06-15T00:00:00Z");
        let r = fixture("cert-expiring", json!({"team": "web"}), data, now());
        let report = evaluate_acm_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_EXPIRING_SOON]);
    }

    #[test]
    fn resilience_does_not_flag_far_future_expiry() {
        let r = fixture("cert-future", json!({"team": "web"}), healthy_data(), now());
        let report = evaluate_acm_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_unparseable_expiry() {
        let mut data = healthy_data();
        data["not_after"] = json!("not-a-date");
        let r = fixture("cert-bad-date", json!({"team": "web"}), data, now());
        let report = evaluate_acm_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_EXPIRY_DATA_UNPARSEABLE]);
        assert_eq!(
            report.findings[0].evidence["not_after"],
            json!("not-a-date")
        );
    }

    #[test]
    fn resilience_flags_imported_renewal_ineligible() {
        let mut data = healthy_data();
        data["certificate_type"] = json!("IMPORTED");
        data["renewal_eligibility"] = json!("INELIGIBLE");
        let r = fixture("cert-imported", json!({"team": "web"}), data, now());
        let report = evaluate_acm_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_RENEWAL_INELIGIBLE]);

        // An Amazon-issued certificate marked INELIGIBLE is not flagged;
        // managed renewal concerns apply to imported certificates only.
        let mut amazon_data = healthy_data();
        amazon_data["renewal_eligibility"] = json!("INELIGIBLE");
        let amazon = fixture("cert-amazon", json!({"team": "web"}), amazon_data, now());
        let report = evaluate_acm_fleet(&[amazon], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn stale_inventory_is_reported() {
        let mut r = fixture("cert-stale", json!({"team": "web"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_acm_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn skips_other_resource_types() {
        let mut r = fixture("vpc-123", json!({}), json!({}), now());
        r.resource_type = "Vpc".to_string();
        let report = evaluate_acm_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_certificate_passes_all_pillars() {
        let r = fixture("cert-ok", json!({"team": "web"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_acm_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.score, 100);
        }
    }
}
