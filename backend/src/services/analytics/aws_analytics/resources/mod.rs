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


pub mod dynamodb;
pub mod ec2;
pub mod elasticache;
pub mod kinesis;
pub mod rds;
pub mod s3;

pub use dynamodb::DynamoDbAnalyzer;
pub use ec2::Ec2Analyzer;
pub use elasticache::ElastiCacheAnalyzer;
pub use kinesis::KinesisAnalyzer;
pub use rds::RdsAnalyzer;
pub use s3::S3Analyzer;
