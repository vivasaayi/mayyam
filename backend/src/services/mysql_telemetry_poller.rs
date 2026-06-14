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

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::repositories::database::DatabaseRepository;
use crate::repositories::mysql_telemetry_snapshot_repository::MySqlTelemetrySnapshotRepository;
use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetryCollector;
use crate::utils::database::connect_to_dynamic_database;

pub struct MySqlTelemetryPoller {
    db: Arc<DatabaseConnection>,
    config: Config,
    interval_seconds: u64,
    max_connections_per_cycle: usize,
}

impl MySqlTelemetryPoller {
    pub fn new(
        db: Arc<DatabaseConnection>,
        config: Config,
        interval_seconds: u64,
        max_connections_per_cycle: usize,
    ) -> Self {
        Self {
            db,
            config,
            interval_seconds: interval_seconds.max(30),
            max_connections_per_cycle: max_connections_per_cycle.max(1),
        }
    }

    pub async fn start(self: Arc<Self>) {
        let mut ticker = interval(TokioDuration::from_secs(self.interval_seconds));
        info!(
            interval_seconds = self.interval_seconds,
            max_connections_per_cycle = self.max_connections_per_cycle,
            "Starting MySQL telemetry poller"
        );

        loop {
            ticker.tick().await;
            if let Err(error) = self.poll_once().await {
                error!(error = %error, "MySQL telemetry poller cycle failed");
            }
        }
    }

    async fn poll_once(&self) -> Result<(), String> {
        let database_repo = DatabaseRepository::new(self.db.clone(), self.config.clone());
        let telemetry_repo = MySqlTelemetrySnapshotRepository::new(self.db.clone());
        let connections = database_repo
            .find_all()
            .await
            .map_err(|e| format!("Failed to list database connections: {}", e))?;

        let mysql_connections = connections
            .into_iter()
            .filter(|connection| {
                let connection_type = connection.connection_type.to_lowercase();
                connection_type == "mysql" || connection_type == "aurora-mysql"
            })
            .take(self.max_connections_per_cycle);

        let mut attempted = 0usize;
        let mut persisted = 0usize;

        for connection in mysql_connections {
            attempted += 1;
            match connect_to_dynamic_database(&connection, &self.config).await {
                Ok(dynamic_conn) => match MySqlTelemetryCollector::collect(&dynamic_conn).await {
                    Ok(snapshot) => {
                        if let Err(error) = telemetry_repo
                            .create_from_snapshot(connection.id, &snapshot)
                            .await
                        {
                            warn!(
                                connection_id = %connection.id,
                                connection_name = %connection.name,
                                error = %error,
                                "Failed to persist scheduled MySQL telemetry snapshot"
                            );
                        } else {
                            persisted += 1;
                        }
                    }
                    Err(error) => {
                        warn!(
                            connection_id = %connection.id,
                            connection_name = %connection.name,
                            error = %error,
                            "Failed to collect scheduled MySQL telemetry"
                        );
                    }
                },
                Err(error) => {
                    warn!(
                        connection_id = %connection.id,
                        connection_name = %connection.name,
                        error = %error,
                        "Failed to connect for scheduled MySQL telemetry"
                    );
                }
            }
        }

        debug!(
            attempted_connections = attempted,
            persisted_snapshots = persisted,
            "Completed MySQL telemetry poller cycle"
        );

        Ok(())
    }
}
