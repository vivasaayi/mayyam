use crate::errors::AppError;
use crate::models::aws_cost_data::CostDataModel;
use crate::models::aws_resource::Model as AwsResourceModel;
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::repositories::cost_analytics::CostAnalyticsRepository;
use chrono::NaiveDate;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedCostData {
    pub cost_data: CostDataModel,
    pub resource_metadata: Option<ResourceMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    pub resource_id: String,
    pub name: Option<String>,
    pub resource_type: String,
    pub region: String,
    pub arn: String,
    pub tags: serde_json::Value,
    pub resource_data: serde_json::Value,
    pub cost_allocation_tags: HashMap<String, String>,
}

#[derive(Debug)]
pub struct ResourceCostEnrichmentService {
    db: Arc<DatabaseConnection>,
    cost_repo: Arc<CostAnalyticsRepository>,
    resource_repo: Arc<AwsResourceRepository>,
}

impl ResourceCostEnrichmentService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        cost_repo: Arc<CostAnalyticsRepository>,
        resource_repo: Arc<AwsResourceRepository>,
    ) -> Self {
        Self {
            db,
            cost_repo,
            resource_repo,
        }
    }

    /// Enrich cost data with resource metadata for better cost attribution
    pub async fn enrich_cost_data(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        resource_id_filter: Option<&str>,
    ) -> Result<Vec<EnrichedCostData>, AppError> {
        tracing::info!(
            "Enriching cost data with resource metadata for account {} from {} to {}",
            account_id,
            start_date,
            end_date
        );

        // Get cost data
        let cost_data = self.cost_repo.get_resource_costs(
            account_id,
            start_date,
            end_date,
            resource_id_filter,
            None, // service_name
            None, // region
            None, // availability_zone
            None, // instance_type
            Some(1000), // limit
        ).await?;

        // Extract unique resource IDs from cost data
        let resource_ids: Vec<String> = cost_data
            .iter()
            .filter_map(|cost| {
                cost.tags.as_ref()
                    .and_then(|t| t.get("resource_id"))
                    .and_then(|rid| rid.as_str())
                    .map(|s| s.to_string())
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Get resource metadata for these resource IDs
        let resource_metadata = if !resource_ids.is_empty() {
            self.resource_repo
                .get_resources_by_ids(account_id, &resource_ids)
                .await?
                .into_iter()
                .map(|r| (r.resource_id.clone(), r))
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };

        // Enrich cost data with resource metadata
        let enriched_data = cost_data
            .into_iter()
            .map(|cost| {
                let resource_metadata = cost.tags.as_ref()
                    .and_then(|t| t.get("resource_id"))
                    .and_then(|rid| rid.as_str())
                    .and_then(|rid| resource_metadata.get(rid))
                    .map(|resource| {
                        // Extract cost allocation tags from resource tags
                        let cost_allocation_tags = self.extract_cost_allocation_tags(&resource.tags);

                        ResourceMetadata {
                            resource_id: resource.resource_id.clone(),
                            name: resource.name.clone(),
                            resource_type: resource.resource_type.clone(),
                            region: resource.region.clone(),
                            arn: resource.arn.clone(),
                            tags: resource.tags.clone(),
                            resource_data: resource.resource_data.clone(),
                            cost_allocation_tags,
                        }
                    });

                EnrichedCostData {
                    cost_data: cost,
                    resource_metadata,
                }
            })
            .collect();

        Ok(enriched_data)
    }

    /// Extract cost allocation tags from resource tags
    fn extract_cost_allocation_tags(&self, resource_tags: &serde_json::Value) -> HashMap<String, String> {
        let mut cost_tags = HashMap::new();

        if let Some(tags_obj) = resource_tags.as_object() {
            // Common cost allocation tag keys
            let cost_tag_keys = [
                "Environment", "Project", "Team", "CostCenter", "Application",
                "Owner", "Department", "BusinessUnit", "Product", "Service"
            ];

            for key in &cost_tag_keys {
                if let Some(value) = tags_obj.get(key).and_then(|v| v.as_str()) {
                    cost_tags.insert(key.to_string(), value.to_string());
                }
            }
        }

        cost_tags
    }

    /// Get cost breakdown by cost allocation tags
    pub async fn get_cost_by_allocation_tags(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        tag_key: &str,
    ) -> Result<HashMap<String, f64>, AppError> {
        tracing::info!(
            "Getting cost breakdown by allocation tag '{}' for account {} from {} to {}",
            tag_key,
            account_id,
            start_date,
            end_date
        );

        let enriched_data = self.enrich_cost_data(account_id, start_date, end_date, None).await?;

        let mut cost_by_tag = HashMap::new();

        for enriched in enriched_data {
            if let Some(metadata) = &enriched.resource_metadata {
                if let Some(tag_value) = metadata.cost_allocation_tags.get(tag_key) {
                    let cost = enriched.cost_data.unblended_cost.to_f64().unwrap_or(0.0);
                    *cost_by_tag.entry(tag_value.clone()).or_insert(0.0) += cost;
                }
            }
        }

        Ok(cost_by_tag)
    }

    /// Get resources with highest costs and their metadata
    pub async fn get_top_cost_resources(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        limit: usize,
    ) -> Result<Vec<EnrichedCostData>, AppError> {
        tracing::info!(
            "Getting top {} cost resources with metadata for account {} from {} to {}",
            limit,
            account_id,
            start_date,
            end_date
        );

        let mut enriched_data = self.enrich_cost_data(account_id, start_date, end_date, None).await?;

        // Sort by cost descending
        enriched_data.sort_by(|a, b| {
            let cost_a = a.cost_data.unblended_cost.to_f64().unwrap_or(0.0);
            let cost_b = b.cost_data.unblended_cost.to_f64().unwrap_or(0.0);
            cost_b.partial_cmp(&cost_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top N
        enriched_data.truncate(limit);

        Ok(enriched_data)
    }
}