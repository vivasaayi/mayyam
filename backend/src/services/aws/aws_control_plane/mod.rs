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


pub mod api_gateway_control_plane;
pub mod cloudfront_control_plane;
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
pub mod rds_control_plane;
pub mod s3_control_plane;
pub mod sns_control_plane;
pub mod sqs_control_plane;
pub mod vpc_control_plane;
// Batch 2: Security & Compliance
pub mod kms_control_plane;
pub mod acm_control_plane;
pub mod cloudtrail_control_plane;
pub mod config_control_plane;
// Batch 3: Containers & Serverless
pub mod ecs_control_plane;
pub mod eks_control_plane;
pub mod apprunner_control_plane;
pub mod batch_control_plane;
// Batch 4: Management & Monitoring
pub mod cloudwatch_control_plane;
pub mod ssm_control_plane;
// Batch 5: Application Integration
pub mod eventbridge_control_plane;
pub mod stepfunctions_control_plane;
pub mod ses_control_plane;
// Batch 6: Analytics & Big Data
pub mod redshift_control_plane;
pub mod emr_control_plane;
pub mod athena_control_plane;
pub mod glue_control_plane;
// Batch 7: Edge & DR
pub mod waf_control_plane;
pub mod globalaccelerator_control_plane;
pub mod backup_control_plane;
