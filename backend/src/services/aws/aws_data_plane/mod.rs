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


pub mod dynamodb_data_plane;
pub mod ec2_data_plane;
pub mod elasticache_data_plane;
pub mod kinesis_data_plane;
pub mod lambda_data_plane;
pub mod opensearch_data_plane;
pub mod rds_data_plane;
pub mod s3_data_plane;
pub mod sns_data_plane;
pub mod sqs_data_plane;

pub mod cloudwatch;
pub mod cost_explorer;
