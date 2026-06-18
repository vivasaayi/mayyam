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

// Thin controller for deterministic inventory pillar reports. All scoring
// logic lives in services::aws::inventory; this layer only loads resources
// and shapes the HTTP response with freshness metadata.

use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::debug;

use crate::errors::AppError;
use crate::models::aws_resource::AwsResourceType;
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::services::aws::inventory::acm_pillar_evaluator::evaluate_acm_fleet;
use crate::services::aws::inventory::amazonmq_pillar_evaluator::evaluate_amazonmq_fleet;
use crate::services::aws::inventory::api_gateway_pillar_evaluator::evaluate_api_gateway_fleet;
use crate::services::aws::inventory::apprunner_pillar_evaluator::evaluate_apprunner_fleet;
use crate::services::aws::inventory::appsync_pillar_evaluator::evaluate_appsync_fleet;
use crate::services::aws::inventory::athena_pillar_evaluator::evaluate_athena_fleet;
use crate::services::aws::inventory::aurora_pillar_evaluator::evaluate_aurora_fleet;
use crate::services::aws::inventory::autoscaling_pillar_evaluator::evaluate_autoscaling_fleet;
use crate::services::aws::inventory::backup_pillar_evaluator::evaluate_backup_fleet;
use crate::services::aws::inventory::batch_pillar_evaluator::evaluate_batch_fleet;
use crate::services::aws::inventory::bedrock_pillar_evaluator::evaluate_bedrock_fleet;
use crate::services::aws::inventory::cloudfront_pillar_evaluator::evaluate_cloudfront_fleet;
use crate::services::aws::inventory::cloudtrail_pillar_evaluator::evaluate_cloudtrail_fleet;
use crate::services::aws::inventory::cloudwatch_log_group_pillar_evaluator::evaluate_cloudwatch_log_group_fleet;
use crate::services::aws::inventory::cloudwatch_metric_pillar_evaluator::evaluate_cloudwatch_metric_fleet;
use crate::services::aws::inventory::cloudwatch_pillar_evaluator::evaluate_cloudwatch_fleet;
use crate::services::aws::inventory::comprehend_pillar_evaluator::evaluate_comprehend_fleet;
use crate::services::aws::inventory::computeoptimizer_pillar_evaluator::evaluate_computeoptimizer_fleet;
use crate::services::aws::inventory::config_pillar_evaluator::evaluate_config_fleet;
use crate::services::aws::inventory::controltower_pillar_evaluator::evaluate_controltower_fleet;
use crate::services::aws::inventory::datasync_pillar_evaluator::evaluate_datasync_fleet;
use crate::services::aws::inventory::dms_pillar_evaluator::evaluate_dms_fleet;
use crate::services::aws::inventory::documentdb_pillar_evaluator::evaluate_documentdb_fleet;
use crate::services::aws::inventory::drs_pillar_evaluator::evaluate_drs_fleet;
use crate::services::aws::inventory::dynamodb_pillar_evaluator::evaluate_dynamodb_fleet;
use crate::services::aws::inventory::ebs_pillar_evaluator::evaluate_ebs_fleet;
use crate::services::aws::inventory::ec2_pillar_evaluator::{
    ec2_cost_posture_summary, ec2_cost_triage_context, evaluate_ec2_fleet,
};
use crate::services::aws::inventory::ecs_pillar_evaluator::evaluate_ecs_fleet;
use crate::services::aws::inventory::efs_pillar_evaluator::evaluate_efs_fleet;
use crate::services::aws::inventory::eks_pillar_evaluator::evaluate_eks_fleet;
use crate::services::aws::inventory::elasticache_pillar_evaluator::evaluate_elasticache_fleet;
use crate::services::aws::inventory::elasticbeanstalk_pillar_evaluator::evaluate_elasticbeanstalk_fleet;
use crate::services::aws::inventory::emr_pillar_evaluator::evaluate_emr_fleet;
use crate::services::aws::inventory::eventbridge_pillar_evaluator::evaluate_eventbridge_fleet;
use crate::services::aws::inventory::fargate_pillar_evaluator::evaluate_fargate_fleet;
use crate::services::aws::inventory::firehose_pillar_evaluator::evaluate_firehose_fleet;
use crate::services::aws::inventory::fsx_pillar_evaluator::evaluate_fsx_fleet;
use crate::services::aws::inventory::glacier_pillar_evaluator::evaluate_glacier_fleet;
use crate::services::aws::inventory::globalaccelerator_pillar_evaluator::evaluate_globalaccelerator_fleet;
use crate::services::aws::inventory::glue_pillar_evaluator::evaluate_glue_fleet;
use crate::services::aws::inventory::guardduty_pillar_evaluator::evaluate_guardduty_fleet;
use crate::services::aws::inventory::health_pillar_evaluator::evaluate_health_fleet;
use crate::services::aws::inventory::iam_pillar_evaluator::evaluate_iam_fleet;
use crate::services::aws::inventory::inspector_pillar_evaluator::evaluate_inspector_fleet;
use crate::services::aws::inventory::internet_gateway_pillar_evaluator::evaluate_internet_gateway_fleet;
use crate::services::aws::inventory::kinesis_pillar_evaluator::evaluate_kinesis_fleet;
use crate::services::aws::inventory::kinesisanalytics_pillar_evaluator::evaluate_kinesisanalytics_fleet;
use crate::services::aws::inventory::kms_pillar_evaluator::evaluate_kms_fleet;
use crate::services::aws::inventory::lakeformation_pillar_evaluator::evaluate_lakeformation_fleet;
use crate::services::aws::inventory::lambda_pillar_evaluator::evaluate_lambda_fleet;
use crate::services::aws::inventory::lightsail_pillar_evaluator::evaluate_lightsail_fleet;
use crate::services::aws::inventory::load_balancer_pillar_evaluator::evaluate_load_balancer_fleet;
use crate::services::aws::inventory::macie_pillar_evaluator::evaluate_macie_fleet;
use crate::services::aws::inventory::memorydb_pillar_evaluator::evaluate_memorydb_fleet;
use crate::services::aws::inventory::mgn_pillar_evaluator::evaluate_mgn_fleet;
use crate::services::aws::inventory::msk_pillar_evaluator::evaluate_msk_fleet;
use crate::services::aws::inventory::nat_gateway_pillar_evaluator::evaluate_nat_gateway_fleet;
use crate::services::aws::inventory::neptune_pillar_evaluator::evaluate_neptune_fleet;
use crate::services::aws::inventory::network_acl_pillar_evaluator::evaluate_network_acl_fleet;
use crate::services::aws::inventory::opensearch_pillar_evaluator::evaluate_opensearch_fleet;
use crate::services::aws::inventory::organizations_pillar_evaluator::evaluate_organizations_fleet;
use crate::services::aws::inventory::privatelink_pillar_evaluator::evaluate_privatelink_fleet;
use crate::services::aws::inventory::quicksight_pillar_evaluator::evaluate_quicksight_fleet;
use crate::services::aws::inventory::rds_pillar_evaluator::evaluate_rds_fleet;
use crate::services::aws::inventory::redshift_pillar_evaluator::evaluate_redshift_fleet;
use crate::services::aws::inventory::resiliencehub_pillar_evaluator::evaluate_resiliencehub_fleet;
use crate::services::aws::inventory::route53_pillar_evaluator::evaluate_route53_fleet;
use crate::services::aws::inventory::route_table_pillar_evaluator::evaluate_route_table_fleet;
use crate::services::aws::inventory::s3_pillar_evaluator::evaluate_s3_fleet;
use crate::services::aws::inventory::sagemaker_pillar_evaluator::evaluate_sagemaker_fleet;
use crate::services::aws::inventory::secretsmanager_pillar_evaluator::evaluate_secretsmanager_fleet;
use crate::services::aws::inventory::security_group_pillar_evaluator::evaluate_security_group_fleet;
use crate::services::aws::inventory::securityhub_pillar_evaluator::evaluate_securityhub_fleet;
use crate::services::aws::inventory::servicecatalog_pillar_evaluator::evaluate_servicecatalog_fleet;
use crate::services::aws::inventory::shield_pillar_evaluator::evaluate_shield_fleet;
use crate::services::aws::inventory::sns_pillar_evaluator::evaluate_sns_fleet;
use crate::services::aws::inventory::sqs_pillar_evaluator::evaluate_sqs_fleet;
use crate::services::aws::inventory::ssm_pillar_evaluator::evaluate_ssm_fleet;
use crate::services::aws::inventory::stepfunctions_pillar_evaluator::evaluate_stepfunctions_fleet;
use crate::services::aws::inventory::storagegateway_pillar_evaluator::evaluate_storagegateway_fleet;
use crate::services::aws::inventory::subnet_pillar_evaluator::evaluate_subnet_fleet;
use crate::services::aws::inventory::textract_pillar_evaluator::evaluate_textract_fleet;
use crate::services::aws::inventory::timestream_pillar_evaluator::evaluate_timestream_fleet;
use crate::services::aws::inventory::transitgateway_pillar_evaluator::evaluate_transitgateway_fleet;
use crate::services::aws::inventory::trustedadvisor_pillar_evaluator::evaluate_trustedadvisor_fleet;
use crate::services::aws::inventory::types::{Pillar, DEFAULT_STALE_AFTER_HOURS};
use crate::services::aws::inventory::vpc_pillar_evaluator::evaluate_vpc_fleet;
use crate::services::aws::inventory::waf_pillar_evaluator::evaluate_waf_fleet;

#[derive(Clone)]
pub struct AwsInventoryController {
    aws_resource_repo: Arc<AwsResourceRepository>,
}

impl AwsInventoryController {
    pub fn new(aws_resource_repo: Arc<AwsResourceRepository>) -> Self {
        Self { aws_resource_repo }
    }
}

#[derive(Debug, Deserialize)]
pub struct Ec2PillarQuery {
    pub account_id: String,
    /// Optional pillar name (e.g. `cost`, `security`, `resilience`,
    /// `performance`). Omit for every pillar the service supports.
    pub pillar: Option<String>,
}

/// Pillars every inventory evaluator implements.
const BASE_PILLARS: &[Pillar] = &[Pillar::Cost, Pillar::Security, Pillar::Resilience];
/// EC2 has M2 telemetry coverage for performance in addition to its M1 pillars.
const EC2_PILLARS: &[Pillar] = &[
    Pillar::Cost,
    Pillar::Security,
    Pillar::Resilience,
    Pillar::Performance,
    Pillar::Scalability,
    Pillar::DisasterRecovery,
    Pillar::OperationalExcellence,
];
/// Full pillar set for services with extended evaluator coverage.
const ALL_PILLARS: &[Pillar] = &[
    Pillar::Cost,
    Pillar::Security,
    Pillar::Resilience,
    Pillar::Performance,
    Pillar::Scalability,
    Pillar::DisasterRecovery,
    Pillar::OperationalExcellence,
];

fn parse_pillars(raw: &Option<String>, supported: &[Pillar]) -> Result<Vec<Pillar>, AppError> {
    let supported_names = || {
        supported
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    match raw {
        Some(raw) => {
            let pillar = Pillar::parse(raw).ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Unknown pillar '{}'; expected one of: {}",
                    raw,
                    supported_names()
                ))
            })?;
            if !supported.contains(&pillar) {
                return Err(AppError::BadRequest(format!(
                    "Pillar '{}' is not supported for this service yet; expected one of: {}",
                    raw,
                    supported_names()
                )));
            }
            Ok(vec![pillar])
        }
        None => Ok(supported.to_vec()),
    }
}

async fn pillar_reports(
    controller: &AwsInventoryController,
    query: Ec2PillarQuery,
    resource_type: AwsResourceType,
    evaluate: impl Fn(
        &[crate::models::aws_resource::Model],
        Pillar,
        chrono::DateTime<Utc>,
    ) -> crate::services::aws::inventory::types::PillarReport,
) -> Result<HttpResponse, AppError> {
    pillar_reports_for(controller, query, resource_type, BASE_PILLARS, evaluate).await
}

/// Same as [`pillar_reports`] for services whose evaluator also covers the
/// performance, scalability, disaster-recovery, and operational-excellence
/// pillars.
async fn extended_pillar_reports(
    controller: &AwsInventoryController,
    query: Ec2PillarQuery,
    resource_type: AwsResourceType,
    evaluate: impl Fn(
        &[crate::models::aws_resource::Model],
        Pillar,
        chrono::DateTime<Utc>,
    ) -> crate::services::aws::inventory::types::PillarReport,
) -> Result<HttpResponse, AppError> {
    pillar_reports_for(controller, query, resource_type, ALL_PILLARS, evaluate).await
}

async fn pillar_reports_for(
    controller: &AwsInventoryController,
    query: Ec2PillarQuery,
    resource_type: AwsResourceType,
    supported: &[Pillar],
    evaluate: impl Fn(
        &[crate::models::aws_resource::Model],
        Pillar,
        chrono::DateTime<Utc>,
    ) -> crate::services::aws::inventory::types::PillarReport,
) -> Result<HttpResponse, AppError> {
    let pillars = parse_pillars(&query.pillar, supported)?;
    let resources = controller
        .aws_resource_repo
        .find_by_account_and_type(&query.account_id, &resource_type.to_string())
        .await?;

    let now = Utc::now();
    let reports: Vec<_> = pillars
        .iter()
        .map(|pillar| evaluate(&resources, *pillar, now))
        .collect();
    let oldest_refresh = resources.iter().map(|r| r.last_refreshed).min();

    Ok(HttpResponse::Ok().json(json!({
        "account_id": query.account_id,
        "resource_type": resource_type.to_string(),
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": resources.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

/// Pillar reports that span several persisted resource types (e.g. IAM
/// users/roles/policies/groups) load every type into one fleet slice and
/// report under a combined resource_type label.
async fn multi_type_pillar_reports(
    controller: &AwsInventoryController,
    query: Ec2PillarQuery,
    resource_types: &[AwsResourceType],
    combined_label: &str,
    evaluate: impl Fn(
        &[crate::models::aws_resource::Model],
        Pillar,
        chrono::DateTime<Utc>,
    ) -> crate::services::aws::inventory::types::PillarReport,
) -> Result<HttpResponse, AppError> {
    let pillars = parse_pillars(&query.pillar, BASE_PILLARS)?;

    let mut resources = Vec::new();
    for resource_type in resource_types {
        resources.extend(
            controller
                .aws_resource_repo
                .find_by_account_and_type(&query.account_id, &resource_type.to_string())
                .await?,
        );
    }

    let now = Utc::now();
    let reports: Vec<_> = pillars
        .iter()
        .map(|pillar| evaluate(&resources, *pillar, now))
        .collect();
    let oldest_refresh = resources.iter().map(|r| r.last_refreshed).min();

    Ok(HttpResponse::Ok().json(json!({
        "account_id": query.account_id,
        "resource_type": combined_label,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": resources.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_ec2_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("EC2 pillar report request: {:?}", query);
    let pillars = parse_pillars(&query.pillar, EC2_PILLARS)?;
    let resources = controller
        .aws_resource_repo
        .find_by_account_and_type(&query.account_id, &AwsResourceType::EC2Instance.to_string())
        .await?;

    let now = Utc::now();
    let reports: Vec<_> = pillars
        .iter()
        .map(|pillar| {
            let report = evaluate_ec2_fleet(&resources, *pillar, now);
            if *pillar == Pillar::Cost {
                json!({
                    "pillar": report.pillar,
                    "resources_evaluated": report.resources_evaluated,
                    "stale_resources": report.stale_resources,
                    "score": report.score,
                    "findings": report.findings,
                    "posture": ec2_cost_posture_summary(&report),
                    "triage_context": ec2_cost_triage_context(&report),
                })
            } else {
                json!(report)
            }
        })
        .collect();
    let oldest_refresh = resources.iter().map(|r| r.last_refreshed).min();

    Ok(HttpResponse::Ok().json(json!({
        "account_id": query.account_id,
        "resource_type": AwsResourceType::EC2Instance.to_string(),
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": resources.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_lambda_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Lambda pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::LambdaFunction,
        evaluate_lambda_fleet,
    )
    .await
}

pub async fn get_s3_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("S3 pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::S3Bucket,
        evaluate_s3_fleet,
    )
    .await
}

pub async fn get_rds_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("RDS pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::RdsInstance,
        evaluate_rds_fleet,
    )
    .await
}

pub async fn get_ebs_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("EBS pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::EbsVolume,
        evaluate_ebs_fleet,
    )
    .await
}

pub async fn get_efs_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("EFS pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::EfsFileSystem,
        evaluate_efs_fleet,
    )
    .await
}

/// ECS pillar reports span clusters and services, so this handler loads
/// both resource types before evaluating.
pub async fn get_ecs_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("ECS pillar report request: {:?}", query);
    let pillars = parse_pillars(&query.pillar, BASE_PILLARS)?;

    let mut resources = controller
        .aws_resource_repo
        .find_by_account_and_type(&query.account_id, &AwsResourceType::EcsCluster.to_string())
        .await?;
    resources.extend(
        controller
            .aws_resource_repo
            .find_by_account_and_type(&query.account_id, &AwsResourceType::EcsService.to_string())
            .await?,
    );

    let now = Utc::now();
    let reports: Vec<_> = pillars
        .iter()
        .map(|pillar| evaluate_ecs_fleet(&resources, *pillar, now))
        .collect();
    let oldest_refresh = resources.iter().map(|r| r.last_refreshed).min();

    Ok(HttpResponse::Ok().json(json!({
        "account_id": query.account_id,
        "resource_type": "EcsClusterAndService",
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": resources.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_sqs_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("SQS pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::SqsQueue,
        evaluate_sqs_fleet,
    )
    .await
}

pub async fn get_sns_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("SNS pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::SnsTopics,
        evaluate_sns_fleet,
    )
    .await
}

pub async fn get_kinesis_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Kinesis pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::KinesisStream,
        evaluate_kinesis_fleet,
    )
    .await
}

pub async fn get_elasticache_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("ElastiCache pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::ElasticacheCluster,
        evaluate_elasticache_fleet,
    )
    .await
}

pub async fn get_opensearch_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("OpenSearch pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::OpenSearchDomain,
        evaluate_opensearch_fleet,
    )
    .await
}

pub async fn get_vpc_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("VPC pillar report request: {:?}", query);
    pillar_reports(&controller, query, AwsResourceType::Vpc, evaluate_vpc_fleet).await
}

pub async fn get_dynamodb_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("DynamoDB pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::DynamoDbTable,
        evaluate_dynamodb_fleet,
    )
    .await
}

pub async fn get_eks_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("EKS pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::EksCluster,
        evaluate_eks_fleet,
    )
    .await
}

pub async fn get_iam_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("IAM pillar report request: {:?}", query);
    multi_type_pillar_reports(
        &controller,
        query,
        &[
            AwsResourceType::IamUser,
            AwsResourceType::IamRole,
            AwsResourceType::IamPolicy,
            AwsResourceType::IamGroup,
        ],
        "IamUserRolePolicyAndGroup",
        evaluate_iam_fleet,
    )
    .await
}

pub async fn get_cloudfront_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("CloudFront pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::CloudFrontDistribution,
        evaluate_cloudfront_fleet,
    )
    .await
}

pub async fn get_elb_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("ELB pillar report request: {:?}", query);
    multi_type_pillar_reports(
        &controller,
        query,
        &[
            AwsResourceType::Alb,
            AwsResourceType::Nlb,
            AwsResourceType::Elb,
        ],
        "AlbNlbAndElb",
        evaluate_load_balancer_fleet,
    )
    .await
}

pub async fn get_api_gateway_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("API Gateway pillar report request: {:?}", query);
    multi_type_pillar_reports(
        &controller,
        query,
        &[
            AwsResourceType::ApiGatewayRestApi,
            AwsResourceType::ApiGatewayStage,
            AwsResourceType::ApiGatewayMethod,
        ],
        "ApiGatewayRestApiStageAndMethod",
        evaluate_api_gateway_fleet,
    )
    .await
}

pub async fn get_cloudwatch_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("CloudWatch pillar report request: {:?}", query);
    multi_type_pillar_reports(
        &controller,
        query,
        &[
            AwsResourceType::CloudWatchAlarm,
            AwsResourceType::CloudWatchDashboard,
        ],
        "CloudWatchAlarmAndDashboard",
        evaluate_cloudwatch_fleet,
    )
    .await
}

pub async fn get_appsync_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("AppSync pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::AppSyncApi,
        evaluate_appsync_fleet,
    )
    .await
}

pub async fn get_glacier_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Glacier pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::GlacierArchive,
        evaluate_glacier_fleet,
    )
    .await
}

pub async fn get_storagegateway_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Storage Gateway pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::StorageGateway,
        evaluate_storagegateway_fleet,
    )
    .await
}

pub async fn get_kinesisanalytics_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Kinesis Analytics pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::KinesisAnalyticsApp,
        evaluate_kinesisanalytics_fleet,
    )
    .await
}

pub async fn get_subnet_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Subnet pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::Subnet,
        evaluate_subnet_fleet,
    )
    .await
}

pub async fn get_security_group_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Security group pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::SecurityGroup,
        evaluate_security_group_fleet,
    )
    .await
}

pub async fn get_nat_gateway_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("NAT gateway pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::NatGateway,
        evaluate_nat_gateway_fleet,
    )
    .await
}

pub async fn get_internet_gateway_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Internet gateway pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::InternetGateway,
        evaluate_internet_gateway_fleet,
    )
    .await
}

pub async fn get_route_table_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Route table pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::RouteTable,
        evaluate_route_table_fleet,
    )
    .await
}

pub async fn get_network_acl_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Network ACL pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::NetworkAcl,
        evaluate_network_acl_fleet,
    )
    .await
}

pub async fn get_fargate_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Fargate pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::FargateProfile,
        evaluate_fargate_fleet,
    )
    .await
}

pub async fn get_kms_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("KMS pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::KmsKey,
        evaluate_kms_fleet,
    )
    .await
}

pub async fn get_acm_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("ACM pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::AcmCertificate,
        evaluate_acm_fleet,
    )
    .await
}

pub async fn get_cloudtrail_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("CloudTrail pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::CloudTrailTrail,
        evaluate_cloudtrail_fleet,
    )
    .await
}

pub async fn get_config_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("AWS Config pillar report request: {:?}", query);
    extended_pillar_reports(
        &controller,
        query,
        AwsResourceType::ConfigRule,
        evaluate_config_fleet,
    )
    .await
}

pub async fn get_eventbridge_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("EventBridge pillar report request: {:?}", query);
    extended_pillar_reports(
        &controller,
        query,
        AwsResourceType::EventBridgeRule,
        evaluate_eventbridge_fleet,
    )
    .await
}

pub async fn get_stepfunctions_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Step Functions pillar report request: {:?}", query);
    extended_pillar_reports(
        &controller,
        query,
        AwsResourceType::StepFunction,
        evaluate_stepfunctions_fleet,
    )
    .await
}

pub async fn get_apprunner_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("App Runner pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::AppRunnerService,
        evaluate_apprunner_fleet,
    )
    .await
}

pub async fn get_athena_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Athena pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::AthenaWorkgroup,
        evaluate_athena_fleet,
    )
    .await
}

pub async fn get_ssm_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("SSM document pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::SsmDocument,
        evaluate_ssm_fleet,
    )
    .await
}

pub async fn get_backup_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("AWS Backup pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::BackupVault,
        evaluate_backup_fleet,
    )
    .await
}

pub async fn get_batch_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Batch pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::BatchComputeEnv,
        evaluate_batch_fleet,
    )
    .await
}

pub async fn get_emr_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("EMR pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::EmrCluster,
        evaluate_emr_fleet,
    )
    .await
}

pub async fn get_globalaccelerator_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Global Accelerator pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::GlobalAccelerator,
        evaluate_globalaccelerator_fleet,
    )
    .await
}

pub async fn get_glue_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Glue pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::GlueDatabase,
        evaluate_glue_fleet,
    )
    .await
}

pub async fn get_redshift_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Redshift pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::RedshiftCluster,
        evaluate_redshift_fleet,
    )
    .await
}

pub async fn get_waf_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("WAF pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::WafWebAcl,
        evaluate_waf_fleet,
    )
    .await
}

pub async fn get_autoscaling_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Auto Scaling pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::AutoScalingGroup,
        evaluate_autoscaling_fleet,
    )
    .await
}

pub async fn get_route53_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Route 53 pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::Route53HostedZone,
        evaluate_route53_fleet,
    )
    .await
}

pub async fn get_transitgateway_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Transit Gateway pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::TransitGateway,
        evaluate_transitgateway_fleet,
    )
    .await
}

pub async fn get_secretsmanager_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Secrets Manager pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::SecretsManagerSecret,
        evaluate_secretsmanager_fleet,
    )
    .await
}

pub async fn get_cloudwatch_metric_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("CloudWatch metric pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::CloudWatchMetric,
        evaluate_cloudwatch_metric_fleet,
    )
    .await
}

pub async fn get_cloudwatch_log_group_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("CloudWatch log group pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::CloudWatchLogGroup,
        evaluate_cloudwatch_log_group_fleet,
    )
    .await
}

pub async fn get_aurora_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Aurora pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::AuroraCluster,
        evaluate_aurora_fleet,
    )
    .await
}

pub async fn get_msk_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("MSK pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::MskCluster,
        evaluate_msk_fleet,
    )
    .await
}

pub async fn get_guardduty_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("GuardDuty pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::GuardDutyDetector,
        evaluate_guardduty_fleet,
    )
    .await
}

pub async fn get_securityhub_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Security Hub pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::SecurityHubHub,
        evaluate_securityhub_fleet,
    )
    .await
}

pub async fn get_inspector_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Inspector pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::InspectorAccountCoverage,
        evaluate_inspector_fleet,
    )
    .await
}

pub async fn get_macie_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Macie pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::MacieAccount,
        evaluate_macie_fleet,
    )
    .await
}

pub async fn get_organizations_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Organizations pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::OrganizationsOrganization,
        evaluate_organizations_fleet,
    )
    .await
}

pub async fn get_controltower_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Control Tower pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::ControlTowerLandingZone,
        evaluate_controltower_fleet,
    )
    .await
}

pub async fn get_servicecatalog_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Service Catalog pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::ServiceCatalogPortfolio,
        evaluate_servicecatalog_fleet,
    )
    .await
}

pub async fn get_trustedadvisor_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Trusted Advisor pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::TrustedAdvisorAccount,
        evaluate_trustedadvisor_fleet,
    )
    .await
}

pub async fn get_computeoptimizer_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Compute Optimizer pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::ComputeOptimizerAccount,
        evaluate_computeoptimizer_fleet,
    )
    .await
}

pub async fn get_health_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("AWS Health pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::HealthAccount,
        evaluate_health_fleet,
    )
    .await
}

pub async fn get_resiliencehub_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Resilience Hub pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::ResilienceHubAccount,
        evaluate_resiliencehub_fleet,
    )
    .await
}

pub async fn get_documentdb_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("DocumentDB pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::DocumentDbCluster,
        evaluate_documentdb_fleet,
    )
    .await
}

pub async fn get_neptune_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Neptune pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::NeptuneCluster,
        evaluate_neptune_fleet,
    )
    .await
}

pub async fn get_memorydb_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("MemoryDB pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::MemoryDbCluster,
        evaluate_memorydb_fleet,
    )
    .await
}

pub async fn get_elasticbeanstalk_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Elastic Beanstalk pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::ElasticBeanstalkEnvironment,
        evaluate_elasticbeanstalk_fleet,
    )
    .await
}

pub async fn get_datasync_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("DataSync pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::DataSyncTask,
        evaluate_datasync_fleet,
    )
    .await
}

pub async fn get_fsx_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("FSx pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::FsxFileSystem,
        evaluate_fsx_fleet,
    )
    .await
}

pub async fn get_timestream_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Timestream pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::TimestreamTable,
        evaluate_timestream_fleet,
    )
    .await
}

pub async fn get_firehose_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Firehose pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::FirehoseDeliveryStream,
        evaluate_firehose_fleet,
    )
    .await
}

pub async fn get_lakeformation_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Lake Formation pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::LakeFormationDataLake,
        evaluate_lakeformation_fleet,
    )
    .await
}

pub async fn get_lightsail_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Lightsail pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::LightsailResource,
        evaluate_lightsail_fleet,
    )
    .await
}

pub async fn get_quicksight_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("QuickSight pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::QuickSightAsset,
        evaluate_quicksight_fleet,
    )
    .await
}

pub async fn get_dms_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("DMS pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::DmsResource,
        evaluate_dms_fleet,
    )
    .await
}

pub async fn get_mgn_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!(
        "Application Migration Service pillar report request: {:?}",
        query
    );
    pillar_reports(
        &controller,
        query,
        AwsResourceType::MgnResource,
        evaluate_mgn_fleet,
    )
    .await
}

pub async fn get_drs_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!(
        "Elastic Disaster Recovery pillar report request: {:?}",
        query
    );
    pillar_reports(
        &controller,
        query,
        AwsResourceType::DrsResource,
        evaluate_drs_fleet,
    )
    .await
}

pub async fn get_bedrock_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Bedrock pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::BedrockResource,
        evaluate_bedrock_fleet,
    )
    .await
}

pub async fn get_sagemaker_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("SageMaker AI pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::SageMakerResource,
        evaluate_sagemaker_fleet,
    )
    .await
}

pub async fn get_textract_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Textract pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::TextractResource,
        evaluate_textract_fleet,
    )
    .await
}

pub async fn get_comprehend_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Comprehend pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::ComprehendResource,
        evaluate_comprehend_fleet,
    )
    .await
}

pub async fn get_amazonmq_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Amazon MQ pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::AmazonMqBroker,
        evaluate_amazonmq_fleet,
    )
    .await
}

pub async fn get_privatelink_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("PrivateLink pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::VpcEndpoint,
        evaluate_privatelink_fleet,
    )
    .await
}

pub async fn get_shield_pillar_reports(
    controller: web::Data<Arc<AwsInventoryController>>,
    query: web::Query<Ec2PillarQuery>,
) -> Result<HttpResponse, AppError> {
    let query = query.into_inner();
    debug!("Shield pillar report request: {:?}", query);
    pillar_reports(
        &controller,
        query,
        AwsResourceType::ShieldProtection,
        evaluate_shield_fleet,
    )
    .await
}
