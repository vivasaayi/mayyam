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


use clap::{Parser, Subcommand};
use mayyam::{api, cli, config, utils};
use std::error::Error;

#[derive(Parser)]
#[command(name = "mayyam")]
#[command(about = "A comprehensive toolbox for DevOps and SRE engineers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as a web server
    Server {
        /// Port to listen on
        #[arg(short, long, default_value_t = 8085)]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },

    /// Database operations
    Db {
        #[command(subcommand)]
        command: cli::database::DbCommands,
    },

    /// Kafka operations
    Kafka {
        #[command(subcommand)]
        command: cli::kafka::KafkaCommands,
    },

    /// Cloud provider operations
    Cloud {
        #[command(subcommand)]
        command: cli::cloud::CloudCommands,
    },

    /// Kubernetes operations
    K8s {
        #[command(subcommand)]
        command: cli::kubernetes::K8sCommands,
    },

    /// Chaos engineering operations
    Chaos {
        #[command(subcommand)]
        command: cli::chaos::ChaosCommands,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    utils::logging::init_logger();

    // Load configuration
    let config = config::load_config()?;

    // Parse command line arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Server { port, host } => {
            // Start web server
            api::server::run_server(host, port, config).await?;
        }
        Commands::Db { command } => {
            // Handle database commands
            cli::database::handle_command(command, &config).await?;
        }
        Commands::Kafka { command } => {
            // Handle Kafka commands
            cli::kafka::handle_command(command, &config).await?;
        }
        Commands::Cloud { command } => {
            // Handle cloud provider commands
            cli::cloud::handle_command(command, &config).await?;
        }
        Commands::K8s { command } => {
            // Handle Kubernetes commands
            cli::kubernetes::handle_command(command, &config).await?;
        }
        Commands::Chaos { command } => {
            // Handle chaos engineering commands
            cli::chaos::handle_command(command, &config).await?;
        }
    }

    Ok(())
}
