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

// Deterministic inventory pillar evaluators. These run without an LLM,
// emit reason-coded findings, and preserve the raw evidence they used.

pub mod acm_pillar_evaluator;
pub mod amazonmq_pillar_evaluator;
pub mod api_gateway_pillar_evaluator;
pub mod apprunner_pillar_evaluator;
pub mod appsync_pillar_evaluator;
pub mod athena_pillar_evaluator;
pub mod aurora_pillar_evaluator;
pub mod autoscaling_pillar_evaluator;
pub mod backup_pillar_evaluator;
pub mod batch_pillar_evaluator;
pub mod bedrock_pillar_evaluator;
pub mod cloudfront_pillar_evaluator;
pub mod cloudtrail_pillar_evaluator;
pub mod cloudwatch_log_group_pillar_evaluator;
pub mod cloudwatch_metric_pillar_evaluator;
pub mod cloudwatch_pillar_evaluator;
pub mod comprehend_pillar_evaluator;
pub mod computeoptimizer_pillar_evaluator;
pub mod config_pillar_evaluator;
pub mod controltower_pillar_evaluator;
pub mod datasync_pillar_evaluator;
pub mod dms_pillar_evaluator;
pub mod documentdb_pillar_evaluator;
pub mod drs_pillar_evaluator;
pub mod dynamodb_pillar_evaluator;
pub mod ebs_pillar_evaluator;
pub mod ec2_pillar_evaluator;
pub mod ecs_pillar_evaluator;
pub mod efs_pillar_evaluator;
pub mod eks_pillar_evaluator;
pub mod elasticache_pillar_evaluator;
pub mod elasticbeanstalk_pillar_evaluator;
pub mod emr_pillar_evaluator;
pub mod eventbridge_pillar_evaluator;
pub mod fargate_pillar_evaluator;
pub mod firehose_pillar_evaluator;
pub mod fsx_pillar_evaluator;
pub mod glacier_pillar_evaluator;
pub mod globalaccelerator_pillar_evaluator;
pub mod glue_pillar_evaluator;
pub mod guardduty_pillar_evaluator;
pub mod health_pillar_evaluator;
pub mod iam_pillar_evaluator;
pub mod inspector_pillar_evaluator;
pub mod internet_gateway_pillar_evaluator;
pub mod kinesis_pillar_evaluator;
pub mod kinesisanalytics_pillar_evaluator;
pub mod kms_pillar_evaluator;
pub mod lakeformation_pillar_evaluator;
pub mod lambda_pillar_evaluator;
pub mod lightsail_pillar_evaluator;
pub mod load_balancer_pillar_evaluator;
pub mod macie_pillar_evaluator;
pub mod memorydb_pillar_evaluator;
pub mod mgn_pillar_evaluator;
pub mod msk_pillar_evaluator;
pub mod nat_gateway_pillar_evaluator;
pub mod neptune_pillar_evaluator;
pub mod network_acl_pillar_evaluator;
pub mod opensearch_pillar_evaluator;
pub mod organizations_pillar_evaluator;
pub mod privatelink_pillar_evaluator;
pub mod quicksight_pillar_evaluator;
pub mod rds_pillar_evaluator;
pub mod redshift_pillar_evaluator;
pub mod resiliencehub_pillar_evaluator;
pub mod route53_pillar_evaluator;
pub mod route_table_pillar_evaluator;
pub mod s3_pillar_evaluator;
pub mod sagemaker_pillar_evaluator;
pub mod secretsmanager_pillar_evaluator;
pub mod security_group_pillar_evaluator;
pub mod securityhub_pillar_evaluator;
pub mod servicecatalog_pillar_evaluator;
pub mod shield_pillar_evaluator;
pub mod sns_pillar_evaluator;
pub mod sqs_pillar_evaluator;
pub mod ssm_pillar_evaluator;
pub mod stepfunctions_pillar_evaluator;
pub mod storagegateway_pillar_evaluator;
pub mod subnet_pillar_evaluator;
pub mod textract_pillar_evaluator;
pub mod timestream_pillar_evaluator;
pub mod transitgateway_pillar_evaluator;
pub mod trustedadvisor_pillar_evaluator;
pub mod types;
pub mod vpc_pillar_evaluator;
pub mod waf_pillar_evaluator;
