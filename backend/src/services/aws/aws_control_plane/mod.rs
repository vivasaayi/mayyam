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

pub mod amazonmq_control_plane;
pub mod api_gateway_control_plane;
pub mod bedrock_control_plane;
pub mod cloudfront_control_plane;
pub mod comprehend_control_plane;
pub mod dynamodb_control_plane;
pub mod ebs_control_plane;
pub mod ec2_control_plane;
pub mod efs_control_plane;
pub mod elasticache_control_plane;
pub mod iam_control_plane;
pub mod kinesis_control_plane;
pub mod lambda_control_plane;
pub mod load_balancer_control_plane;
pub mod opensearch_control_plane;
pub mod quicksight_control_plane;
pub mod rds_control_plane;
pub mod s3_control_plane;
pub mod sagemaker_control_plane;
pub mod sns_control_plane;
pub mod sqs_control_plane;
pub mod textract_control_plane;
pub mod vpc_control_plane;
// Batch 2: Security & Compliance
pub mod acm_control_plane;
pub mod cloudtrail_control_plane;
pub mod config_control_plane;
pub mod kms_control_plane;
// Batch 3: Containers & Serverless
pub mod apprunner_control_plane;
pub mod batch_control_plane;
pub mod ecs_control_plane;
pub mod eks_control_plane;
// Batch 4: Management & Monitoring
pub mod cloudwatch_control_plane;
pub mod ssm_control_plane;
// Batch 5: Application Integration
pub mod eventbridge_control_plane;
pub mod ses_control_plane;
pub mod stepfunctions_control_plane;
// Batch 6: Analytics & Big Data
pub mod athena_control_plane;
pub mod emr_control_plane;
pub mod glue_control_plane;
pub mod redshift_control_plane;
// Batch 7: Edge & DR
pub mod backup_control_plane;
pub mod globalaccelerator_control_plane;
pub mod shield_control_plane;
pub mod waf_control_plane;
// Final Review Additions
pub mod appsync_control_plane;
pub mod autoscaling_control_plane;
pub mod connect_control_plane;
pub mod glacier_control_plane;
pub mod kinesisanalytics_control_plane;
pub mod storagegateway_control_plane;
// Batch 10: Networking, DNS & Secrets
pub mod privatelink_control_plane;
pub mod route53_control_plane;
pub mod secretsmanager_control_plane;
pub mod transitgateway_control_plane;
// Batch 11: Database Clusters, Streaming & Security Detection
pub mod aurora_control_plane;
pub mod computeoptimizer_control_plane;
pub mod controltower_control_plane;
pub mod guardduty_control_plane;
pub mod health_control_plane;
pub mod inspector_control_plane;
pub mod macie_control_plane;
pub mod msk_control_plane;
pub mod organizations_control_plane;
pub mod resiliencehub_control_plane;
pub mod securityhub_control_plane;
pub mod servicecatalog_control_plane;
pub mod trustedadvisor_control_plane;
// Batch 12: Document DB, Graph DB & In-Memory DB
pub mod documentdb_control_plane;
pub mod memorydb_control_plane;
pub mod neptune_control_plane;
// Batch 13: Platform, Data Movement & File Systems
pub mod datasync_control_plane;
pub mod dms_control_plane;
pub mod drs_control_plane;
pub mod elasticbeanstalk_control_plane;
pub mod firehose_control_plane;
pub mod fsx_control_plane;
pub mod lakeformation_control_plane;
pub mod lightsail_control_plane;
pub mod mgn_control_plane;
pub mod timestream_control_plane;
