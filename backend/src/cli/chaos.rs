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

use clap::Subcommand;
use std::error::Error;

use crate::config::Config;

#[derive(Subcommand)]
pub enum ChaosCommands {
    /// List available chaos experiment templates
    ListTemplates {
        /// Filter by resource type (e.g., EC2Instance, RdsInstance)
        #[arg(short, long)]
        resource_type: Option<String>,

        /// Filter by category (e.g., compute, database, networking)
        #[arg(short, long)]
        category: Option<String>,
    },

    /// List configured chaos experiments
    ListExperiments {
        /// Filter by AWS account ID
        #[arg(short, long)]
        account_id: Option<String>,

        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },

    /// Run a chaos experiment by ID
    Run {
        /// Experiment ID to run
        #[arg(short, long)]
        experiment_id: String,

        /// Run as dry-run (no actual changes)
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Stop a running chaos experiment
    Stop {
        /// Experiment ID to stop
        #[arg(short, long)]
        experiment_id: String,
    },

    /// View experiment history for a resource
    History {
        /// AWS resource ID
        #[arg(short, long)]
        resource_id: String,
    },
}

pub async fn handle_command(command: ChaosCommands, config: &Config) -> Result<(), Box<dyn Error>> {
    match command {
        ChaosCommands::ListTemplates {
            resource_type,
            category,
        } => {
            println!("Chaos Experiment Templates");
            println!("==========================");
            if let Some(ref rt) = resource_type {
                println!("Filtering by resource type: {}", rt);
            }
            if let Some(ref cat) = category {
                println!("Filtering by category: {}", cat);
            }
            println!();
            println!("Available templates (connect to database for full list):");
            println!("  - EC2 Instance Stop (compute/EC2Instance)");
            println!("  - EC2 Instance Reboot (compute/EC2Instance)");
            println!("  - EC2 Instance Terminate (compute/EC2Instance)");
            println!("  - RDS Failover (database/RdsInstance)");
            println!("  - RDS Instance Reboot (database/RdsInstance)");
            println!("  - DynamoDB Table Throttle (database/DynamoDbTable)");
            println!("  - Lambda Function Disable (serverless/LambdaFunction)");
            println!("  - Lambda Function Timeout (serverless/LambdaFunction)");
            println!("  - ECS Service Scale Down (compute/EcsService)");
            println!("  - ElastiCache Failover (database/ElasticacheCluster)");
            println!("  - S3 Bucket Policy Deny (storage/S3Bucket)");
            println!("  - ALB Target Deregistration (networking/Alb)");
            println!("  - Security Group Ingress Block (networking/SecurityGroup)");
            println!("  - SQS Queue Purge (serverless/SqsQueue)");
            println!("  - EKS Node Group Scale Down (compute/EksCluster)");
            Ok(())
        }

        ChaosCommands::ListExperiments { account_id, status } => {
            println!("Chaos Experiments");
            println!("=================");
            println!("Connect to the API server to list configured experiments.");
            println!("Use: GET /api/chaos/experiments");
            Ok(())
        }

        ChaosCommands::Run {
            experiment_id,
            dry_run,
        } => {
            println!("Running chaos experiment: {}", experiment_id);
            if dry_run {
                println!("  Mode: DRY RUN (no actual changes will be made)");
            }
            println!("Connect to the API server to run experiments.");
            println!("Use: POST /api/chaos/experiments/{}/run", experiment_id);
            Ok(())
        }

        ChaosCommands::Stop { experiment_id } => {
            println!("Stopping chaos experiment: {}", experiment_id);
            println!("Connect to the API server to stop experiments.");
            println!("Use: POST /api/chaos/experiments/{}/stop", experiment_id);
            Ok(())
        }

        ChaosCommands::History { resource_id } => {
            println!("Experiment history for resource: {}", resource_id);
            println!("Connect to the API server to view history.");
            println!(
                "Use: GET /api/chaos/resources/{}/history",
                resource_id
            );
            Ok(())
        }
    }
}
