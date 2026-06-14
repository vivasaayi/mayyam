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

// Deterministic S3 Glacier vault inventory evaluators for the cost,
// security, and resilience pillars (roadmap rows 01-AWS-CLOUD-00883/
// 00892/00919).
//
// Evaluates fields persisted by glacier_control_plane: VaultName,
// VaultARN, CreationDate, LastInventoryDate, NumberOfArchives,
// SizeInBytes. The collector persists no vault lock or access policy
// fields, so the security pillar reports an honest data gap per vault.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "GLACIER_COST_NO_TAGS";
pub const REASON_COST_EMPTY_VAULT: &str = "GLACIER_COST_EMPTY_VAULT";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str = "GLACIER_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_RES_NO_VAULT_INVENTORY: &str = "GLACIER_RES_NO_VAULT_INVENTORY";
pub const REASON_INV_STALE_DATA: &str = "GLACIER_INV_STALE_DATA";

/// Evaluate every Glacier vault in the fleet for one pillar.
pub fn evaluate_glacier_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != "GlacierArchive" {
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
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
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
                "Vault {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let archives = resource
        .resource_data
        .get("NumberOfArchives")
        .and_then(|v| v.as_i64());
    if archives == Some(0) {
        let size = resource
            .resource_data
            .get("SizeInBytes")
            .and_then(|v| v.as_i64());
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_EMPTY_VAULT.to_string(),
            severity: Severity::Low,
            message: format!(
                "Vault {} holds zero archives; an empty provisioned vault is a sprawl signal and a candidate for deletion",
                resource.resource_id
            ),
            evidence: json!({ "NumberOfArchives": 0, "SizeInBytes": size }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector persists no vault lock or access policy fields, so
    // security posture cannot be assessed from inventory data yet.
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Vault lock and access policy state for vault {} is not collected yet; security pillar cannot be assessed",
            resource.resource_id
        ),
        evidence: json!({ "security_posture_fields_collected": false }),
    });
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let last_inventory = resource
        .resource_data
        .get("LastInventoryDate")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if last_inventory.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_VAULT_INVENTORY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Vault {} has never completed a vault inventory; archive contents cannot be enumerated for recovery planning",
                resource.resource_id
            ),
            evidence: json!({ "LastInventoryDate": last_inventory }),
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
            resource_type: "GlacierArchive".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:glacier:us-east-1:123456789012:vaults/{}",
                resource_id
            ),
            name: Some(resource_id.to_string()),
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
            "VaultName": "vault-ok",
            "VaultARN": "arn:aws:glacier:us-east-1:123456789012:vaults/vault-ok",
            "CreationDate": "2024-01-15T00:00:00.000Z",
            "LastInventoryDate": "2026-06-01T00:00:00.000Z",
            "NumberOfArchives": 42,
            "SizeInBytes": 1073741824i64,
        })
    }

    #[test]
    fn cost_flags_missing_tags() {
        // The collector always persists {} for tags today, so every vault
        // surfaces this allocation gap until tag collection is added.
        let r = fixture("vault-untagged", json!({}), healthy_data(), now());
        let report = evaluate_glacier_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_NO_TAGS]
        );
    }

    #[test]
    fn cost_flags_empty_vault() {
        let mut data = healthy_data();
        data["NumberOfArchives"] = json!(0);
        data["SizeInBytes"] = json!(0);
        let r = fixture("vault-empty", json!({"team": "archive"}), data, now());
        let report = evaluate_glacier_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_EMPTY_VAULT));
        assert!(!codes.contains(&REASON_COST_NO_TAGS));
    }

    #[test]
    fn security_reports_posture_data_gap_for_every_vault() {
        let r = fixture(
            "vault-gap",
            json!({"team": "archive"}),
            healthy_data(),
            now(),
        );
        let report = evaluate_glacier_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_POSTURE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_missing_vault_inventory() {
        let mut data = healthy_data();
        data["LastInventoryDate"] = json!("");
        let r = fixture("vault-noinv", json!({"team": "archive"}), data, now());
        let report = evaluate_glacier_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_NO_VAULT_INVENTORY]
        );
    }

    #[test]
    fn stale_inventory_row_is_flagged() {
        let mut r = fixture(
            "vault-stale",
            json!({"team": "archive"}),
            healthy_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_glacier_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_glacier_rows_are_skipped() {
        let mut r = fixture("not-a-vault", json!({}), json!({}), now());
        r.resource_type = "S3Bucket".to_string();
        let report = evaluate_glacier_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_vault_passes_cost_and_resilience() {
        // Tags use a non-empty fixture to prove the check logic even though
        // the collector currently persists {}. Security always reports the
        // posture data gap, so it is asserted separately.
        let r = fixture(
            "vault-ok",
            json!({"team": "archive"}),
            healthy_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Resilience] {
            let report = evaluate_glacier_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
        let security = evaluate_glacier_fleet(std::slice::from_ref(&r), Pillar::Security, now());
        assert_eq!(
            security
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_POSTURE_DATA_NOT_COLLECTED]
        );
    }
}
