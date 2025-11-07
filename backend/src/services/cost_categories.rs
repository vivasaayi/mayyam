use crate::errors::AppError;
use crate::models::aws_cost_data::CostDataModel;
use crate::repositories::cost_analytics::CostAnalyticsRepository;
use chrono::NaiveDate;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCategory {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rules: Vec<CostCategoryRule>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCategoryRule {
    pub rule_type: CostCategoryRuleType,
    pub field: String,
    pub operator: CostCategoryOperator,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CostCategoryRuleType {
    Tag,
    Service,
    ResourceType,
    Region,
    InstanceType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CostCategoryOperator {
    Equals,
    Contains,
    StartsWith,
    EndsWith,
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCategoryResult {
    pub category_id: String,
    pub category_name: String,
    pub total_cost: f64,
    pub resource_count: usize,
    pub cost_breakdown: HashMap<String, f64>,
}

#[derive(Debug)]
pub struct CostCategoriesService {
    db: Arc<DatabaseConnection>,
    cost_repo: Arc<CostAnalyticsRepository>,
}

impl CostCategoriesService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        cost_repo: Arc<CostAnalyticsRepository>,
    ) -> Self {
        Self { db, cost_repo }
    }

    /// Create predefined cost categories for common use cases
    pub fn create_default_categories() -> Vec<CostCategory> {
        vec![
            CostCategory {
                id: "compute".to_string(),
                name: "Compute Resources".to_string(),
                description: Some("EC2 instances, Lambda functions, and other compute resources".to_string()),
                rules: vec![
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Equals,
                        value: "Amazon Elastic Compute Cloud - Compute".to_string(),
                    },
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Equals,
                        value: "AWS Lambda".to_string(),
                    },
                ],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            CostCategory {
                id: "storage".to_string(),
                name: "Storage Resources".to_string(),
                description: Some("S3, EBS, RDS storage, and other storage services".to_string()),
                rules: vec![
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Contains,
                        value: "Storage".to_string(),
                    },
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Equals,
                        value: "Amazon Simple Storage Service".to_string(),
                    },
                ],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            CostCategory {
                id: "database".to_string(),
                name: "Database Services".to_string(),
                description: Some("RDS, DynamoDB, and other database services".to_string()),
                rules: vec![
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Contains,
                        value: "Database".to_string(),
                    },
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Equals,
                        value: "Amazon Relational Database Service".to_string(),
                    },
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Equals,
                        value: "Amazon DynamoDB".to_string(),
                    },
                ],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            CostCategory {
                id: "networking".to_string(),
                name: "Networking".to_string(),
                description: Some("VPC, CloudFront, Route 53, and other networking services".to_string()),
                rules: vec![
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Contains,
                        value: "CloudFront".to_string(),
                    },
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Contains,
                        value: "Route 53".to_string(),
                    },
                    CostCategoryRule {
                        rule_type: CostCategoryRuleType::Service,
                        field: "service_name".to_string(),
                        operator: CostCategoryOperator::Contains,
                        value: "VPC".to_string(),
                    },
                ],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        ]
    }

    /// Apply cost categories to cost data and return categorized results
    pub async fn categorize_costs(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        categories: &[CostCategory],
    ) -> Result<Vec<CostCategoryResult>, AppError> {
        tracing::info!(
            "Categorizing costs for account {} from {} to {} using {} categories",
            account_id,
            start_date,
            end_date,
            categories.len()
        );

        // Get all cost data for the period
        let cost_data = self.cost_repo.get_resource_costs(
            account_id,
            start_date,
            end_date,
            None, // resource_id
            None, // service_name
            None, // region
            None, // availability_zone
            None, // instance_type
            Some(10000), // limit - get all data
        ).await?;

        let mut category_results = Vec::new();

        for category in categories {
            let mut category_cost = 0.0;
            let mut resource_count = 0;
            let mut cost_breakdown = HashMap::new();

            for cost_item in &cost_data {
                if self.matches_category(cost_item, category) {
                    let cost = cost_item.unblended_cost.to_f64().unwrap_or(0.0);
                    category_cost += cost;
                    resource_count += 1;

                    // Add to service breakdown within category
                    let service = cost_item.service_name.clone();
                    *cost_breakdown.entry(service).or_insert(0.0) += cost;
                }
            }

            if category_cost > 0.0 {
                category_results.push(CostCategoryResult {
                    category_id: category.id.clone(),
                    category_name: category.name.clone(),
                    total_cost: category_cost,
                    resource_count,
                    cost_breakdown,
                });
            }
        }

        // Sort by total cost descending
        category_results.sort_by(|a, b| b.total_cost.partial_cmp(&a.total_cost).unwrap_or(std::cmp::Ordering::Equal));

        Ok(category_results)
    }

    /// Check if a cost data item matches a category rule
    fn matches_category(&self, cost_item: &CostDataModel, category: &CostCategory) -> bool {
        for rule in &category.rules {
            if self.matches_rule(cost_item, rule) {
                return true;
            }
        }
        false
    }

    /// Check if a cost data item matches a specific rule
    fn matches_rule(&self, cost_item: &CostDataModel, rule: &CostCategoryRule) -> bool {
        let field_value = match rule.rule_type {
            CostCategoryRuleType::Service => Some(cost_item.service_name.clone()),
            CostCategoryRuleType::Region => cost_item.region.clone(),
            CostCategoryRuleType::InstanceType => {
                // Extract instance type from tags if available
                cost_item.tags.as_ref()
                    .and_then(|t| t.get("instance_type"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            },
            CostCategoryRuleType::ResourceType => {
                // This would need to be enriched with resource metadata
                // For now, we'll match based on service name patterns
                Some(cost_item.service_name.clone())
            },
            CostCategoryRuleType::Tag => {
                // Extract tag value from tags JSON
                cost_item.tags.as_ref()
                    .and_then(|t| t.get(&rule.field))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            },
        };

        if let Some(value) = field_value {
            match rule.operator {
                CostCategoryOperator::Equals => value == rule.value,
                CostCategoryOperator::Contains => value.contains(&rule.value),
                CostCategoryOperator::StartsWith => value.starts_with(&rule.value),
                CostCategoryOperator::EndsWith => value.ends_with(&rule.value),
                CostCategoryOperator::Regex => {
                    // Simple regex matching (could be enhanced with proper regex crate)
                    rule.value.split('|').any(|pattern| value.contains(pattern))
                },
            }
        } else {
            false
        }
    }

    /// Create custom cost category based on tag values
    pub fn create_tag_based_category(
        tag_key: &str,
        tag_values: &[String],
        category_name: &str,
        description: Option<String>,
    ) -> CostCategory {
        let rules = tag_values
            .iter()
            .map(|value| CostCategoryRule {
                rule_type: CostCategoryRuleType::Tag,
                field: tag_key.to_string(),
                operator: CostCategoryOperator::Equals,
                value: value.clone(),
            })
            .collect();

        CostCategory {
            id: format!("custom-{}-{}", tag_key.to_lowercase(), chrono::Utc::now().timestamp()),
            name: category_name.to_string(),
            description,
            rules,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// Get cost trends by category over time
    pub async fn get_category_trends(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        categories: &[CostCategory],
        granularity_days: i64,
    ) -> Result<HashMap<String, Vec<(NaiveDate, f64)>>, AppError> {
        tracing::info!(
            "Getting cost trends by category for account {} from {} to {}",
            account_id,
            start_date,
            end_date
        );

        let mut category_trends = HashMap::new();

        // Calculate date ranges
        let mut current_date = start_date;
        while current_date <= end_date {
            let period_end = std::cmp::min(
                current_date + chrono::Duration::days(granularity_days),
                end_date + chrono::Duration::days(1),
            );

            let period_costs = self.categorize_costs(
                account_id,
                current_date,
                period_end - chrono::Duration::days(1),
                categories,
            ).await?;

            for category_result in period_costs {
                category_trends
                    .entry(category_result.category_name)
                    .or_insert_with(Vec::new)
                    .push((current_date, category_result.total_cost));
            }

            current_date = period_end;
        }

        Ok(category_trends)
    }
}