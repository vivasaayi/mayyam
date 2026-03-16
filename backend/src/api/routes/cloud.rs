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


use crate::api::routes::aws_account;
use crate::controllers::cloud;
use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    // General cloud provider routes
    let cloud_scope = web::scope("/api/cloud")
        .route("/providers", web::get().to(cloud::list_providers))
        // Unified multi-cloud resources search
        .route("/resources", web::get().to(cloud::search_cloud_resources));

    // AWS resource management (control plane)
    let aws_scope = web::scope("/api/aws")
        // Resource syncing
        .route("/sync", web::post().to(cloud::sync_aws_resources))
        // Regions listing (DescribeRegions)
        .route("/regions", web::get().to(cloud::list_aws_regions))
        // General resource search
        .route("/resources", web::get().to(cloud::search_aws_resources))
        .route("/resources/{id}", web::get().to(cloud::get_aws_resource))
        // Include AWS account management
        .service(aws_account::configure())
        // EC2 instances
        .route(
            "/accounts/{account_id}/regions/{region}/ec2",
            web::get().to(cloud::list_ec2_instances),
        )
        // ElastiCache/Redis clusters
        .route(
            "/accounts/{account_id}/regions/{region}/elasticache",
            web::get().to(cloud::list_elasticache_clusters),
        )
        // S3 buckets (global resource)
        .route(
            "/accounts/{account_id}/s3",
            web::get().to(cloud::list_s3_buckets),
        )
        // RDS instances
        .route(
            "/accounts/{account_id}/regions/{region}/rds",
            web::get().to(cloud::list_rds_instances),
        )
        // DynamoDB tables
        .route(
            "/accounts/{account_id}/regions/{region}/dynamodb",
            web::get().to(cloud::list_dynamodb_tables),
        )
        // VPC resources
        .route(
            "/accounts/{account_id}/regions/{region}/vpcs",
            web::get().to(cloud::list_vpcs),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/subnets",
            web::get().to(cloud::list_subnets),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/security-groups",
            web::get().to(cloud::list_security_groups),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/internet-gateways",
            web::get().to(cloud::list_internet_gateways),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/nat-gateways",
            web::get().to(cloud::list_nat_gateways),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/route-tables",
            web::get().to(cloud::list_route_tables),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/network-acls",
            web::get().to(cloud::list_network_acls),
        )
        // Load Balancing resources
        .route(
            "/accounts/{account_id}/regions/{region}/albs",
            web::get().to(cloud::list_albs),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/nlbs",
            web::get().to(cloud::list_nlbs),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/elbs",
            web::get().to(cloud::list_elbs),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/cloudfront",
            web::get().to(cloud::list_cloudfront_distributions),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/api-gateway/rest-apis",
            web::get().to(cloud::list_api_gateway_rest_apis),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/api-gateway/stages",
            web::get().to(cloud::list_api_gateway_stages),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/api-gateway/resources",
            web::get().to(cloud::list_api_gateway_resources),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/api-gateway/methods",
            web::get().to(cloud::list_api_gateway_methods),
        )
        // EBS Storage resources
        .route(
            "/accounts/{account_id}/regions/{region}/ebs-volumes",
            web::get().to(cloud::list_ebs_volumes),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/ebs-snapshots",
            web::get().to(cloud::list_ebs_snapshots),
        )
        // EFS Storage resources
        .route(
            "/accounts/{account_id}/regions/{region}/efs-file-systems",
            web::get().to(cloud::list_efs_file_systems),
        )
        // CloudWatch metrics
        .route(
            "/profiles/{profile}/regions/{region}/metrics/{resource_type}/{resource_id}",
            web::get().to(cloud::get_cloudwatch_metrics),
        )
        // CloudWatch logs
        .route(
            "/profiles/{profile}/regions/{region}/logs/{log_group}",
            web::get().to(cloud::get_cloudwatch_logs),
        )
        // Schedule metrics collection
        .route(
            "/profiles/{profile}/regions/{region}/schedule/{resource_type}/{resource_id}",
            web::post().to(cloud::schedule_metrics_collection),
        )
        // AWS cost
        .route(
            "/accounts/{account_id}/regions/{region}/cost",
            web::get().to(cloud::get_aws_cost_and_usage),
        )
        // IAM resources
        .route(
            "/accounts/{account_id}/regions/{region}/iam-users",
            web::get().to(cloud::list_iam_users),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/iam-roles",
            web::get().to(cloud::list_iam_roles),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/iam-policies",
            web::get().to(cloud::list_iam_policies),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/iam-groups",
            web::get().to(cloud::list_iam_groups),
        )
        // SNS Topics
        .route(
            "/accounts/{account_id}/regions/{region}/sns-topics",
            web::get().to(cloud::list_sns_topics),
        )
        // Lambda Functions
        .route(
            "/accounts/{account_id}/regions/{region}/lambda-functions",
            web::get().to(cloud::list_lambda_functions),
        )
        // OpenSearch Domains
        .route(
            "/accounts/{account_id}/regions/{region}/opensearch-domains",
            web::get().to(cloud::list_opensearch_domains),
        )
        // SQS Queues
        .route(
            "/accounts/{account_id}/regions/{region}/sqs-queues",
            web::get().to(cloud::list_sqs_queues),
        )
        // Kinesis Streams
        .route(
            "/accounts/{account_id}/regions/{region}/kinesis-streams",
            web::get().to(cloud::list_kinesis_streams),
        )
        // Batch 2: Security & Compliance
        .route(
            "/accounts/{account_id}/regions/{region}/kms-keys",
            web::get().to(cloud::list_kms_keys),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/acm-certificates",
            web::get().to(cloud::list_acm_certificates),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/cloudtrail-trails",
            web::get().to(cloud::list_cloudtrail_trails),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/config-rules",
            web::get().to(cloud::list_config_rules),
        )
        // Batch 3: Containers & Serverless
        .route(
            "/accounts/{account_id}/regions/{region}/ecs-clusters",
            web::get().to(cloud::list_ecs_clusters),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/ecs-services",
            web::get().to(cloud::list_ecs_services),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/ecs-tasks",
            web::get().to(cloud::list_ecs_tasks),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/eks-clusters",
            web::get().to(cloud::list_eks_clusters),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/eks-fargate-profiles",
            web::get().to(cloud::list_eks_fargate_profiles),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/apprunner-services",
            web::get().to(cloud::list_apprunner_services),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/batch-compute-envs",
            web::get().to(cloud::list_batch_compute_envs),
        )
        // Batch 4: Management & Monitoring
        .route(
            "/accounts/{account_id}/regions/{region}/cloudwatch-alarms",
            web::get().to(cloud::list_cloudwatch_alarms),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/cloudwatch-dashboards",
            web::get().to(cloud::list_cloudwatch_dashboards),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/ssm-documents",
            web::get().to(cloud::list_ssm_documents),
        )
        // Batch 5: Application Integration
        .route(
            "/accounts/{account_id}/regions/{region}/eventbridge-rules",
            web::get().to(cloud::list_eventbridge_rules),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/step-functions",
            web::get().to(cloud::list_step_functions),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/ses-identities",
            web::get().to(cloud::list_ses_identities),
        )
        // Batch 6: Analytics & Big Data
        .route(
            "/accounts/{account_id}/regions/{region}/redshift-clusters",
            web::get().to(cloud::list_redshift_clusters),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/emr-clusters",
            web::get().to(cloud::list_emr_clusters),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/athena-workgroups",
            web::get().to(cloud::list_athena_workgroups),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/glue-databases",
            web::get().to(cloud::list_glue_databases),
        )
        // Batch 7: Edge & DR
        .route(
            "/accounts/{account_id}/regions/{region}/waf-web-acls",
            web::get().to(cloud::list_waf_web_acls),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/global-accelerators",
            web::get().to(cloud::list_global_accelerators),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/backup-vaults",
            web::get().to(cloud::list_backup_vaults),
        )
        // Final Review Additions
        .route(
            "/accounts/{account_id}/regions/{region}/glacier-vaults",
            web::get().to(cloud::list_glacier_vaults),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/storage-gateways",
            web::get().to(cloud::list_storage_gateways),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/connect-instances",
            web::get().to(cloud::list_connect_instances),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/cloudfront-functions",
            web::get().to(cloud::list_cloudfront_functions),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/appsync-apis",
            web::get().to(cloud::list_appsync_apis),
        )
        .route(
            "/accounts/{account_id}/regions/{region}/kinesis-analytics-apps",
            web::get().to(cloud::list_kinesis_analytics_apps),
        );

    // AWS data plane operations
    let aws_data_scope = web::scope("/api/aws-data")
        // S3 operations
        .route(
            "/profiles/{profile}/s3/{bucket}/{key}",
            web::get().to(cloud::s3_get_object),
        )
        .route(
            "/profiles/{profile}/regions/{region}/s3",
            web::post().to(cloud::s3_put_object),
        )
        // DynamoDB operations
        .route(
            "/profiles/{profile}/regions/{region}/dynamodb/{table}/item",
            web::get().to(cloud::dynamodb_get_item),
        )
        .route(
            "/profiles/{profile}/regions/{region}/dynamodb/{table}/item",
            web::post().to(cloud::dynamodb_put_item),
        )
        .route(
            "/profiles/{profile}/regions/{region}/dynamodb/{table}/query",
            web::post().to(cloud::dynamodb_query),
        )
        // SQS operations
        .route(
            "/profiles/{profile}/regions/{region}/sqs/send",
            web::post().to(cloud::sqs_send_message),
        )
        .route(
            "/profiles/{profile}/regions/{region}/sqs/receive",
            web::post().to(cloud::sqs_receive_messages),
        )
        // Kinesis operations
        .route(
            "/profiles/{profile}/regions/{region}/kinesis",
            web::post().to(cloud::kinesis_put_record),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/streams",
            web::post().to(cloud::kinesis_create_stream),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/streams",
            web::delete().to(cloud::kinesis_delete_stream),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/streams/describe",
            web::post().to(cloud::kinesis_describe_stream),
        )
        // New comprehensive Kinesis operations
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/limits",
            web::get().to(cloud::kinesis_describe_limits),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/streams/summary",
            web::post().to(cloud::kinesis_describe_stream_summary),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/streams/retention/increase",
            web::post().to(cloud::kinesis_increase_retention_period),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/streams/retention/decrease",
            web::post().to(cloud::kinesis_decrease_retention_period),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/streams/monitoring/enable",
            web::post().to(cloud::kinesis_enable_enhanced_monitoring),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/streams/monitoring/disable",
            web::post().to(cloud::kinesis_disable_enhanced_monitoring),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/shards",
            web::post().to(cloud::kinesis_list_shards),
        )
        // Kinesis data plane operations
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/records/put",
            web::post().to(cloud::kinesis_put_records),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/records/get",
            web::post().to(cloud::kinesis_get_records),
        )
        .route(
            "/profiles/{profile}/regions/{region}/kinesis/shard-iterator",
            web::post().to(cloud::kinesis_get_shard_iterator),
        );

    // Register the scopes
    cfg.service(cloud_scope);
    cfg.service(aws_scope);
    cfg.service(aws_data_scope);
}
