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


use crate::repositories::aurora_cluster_repository::AuroraClusterRepository;
use crate::services::slow_query_ingestion_service::SlowQueryIngestionService;
use crate::services::aws::service::AwsService;
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{info, error, warn};

pub struct SlowQueryPoller {
    cluster_repo: AuroraClusterRepository,
    ingestion_service: Arc<SlowQueryIngestionService>,
    aws_service: Arc<AwsService>,
}

impl SlowQueryPoller {
    pub fn new(
        cluster_repo: AuroraClusterRepository,
        ingestion_service: Arc<SlowQueryIngestionService>,
        aws_service: Arc<AwsService>,
    ) -> Self {
        Self {
            cluster_repo,
            ingestion_service,
            aws_service,
        }
    }

    pub async fn start(self: Arc<Self>) {
        let mut interval = interval(TokioDuration::from_secs(300)); // Poll every 5 minutes
        
        info!("Starting Aurora Slow Query Poller...");
        
        loop {
            interval.tick().await;
            if let Err(e) = self.poll_all_clusters().await {
                error!("Error polling slow query logs: {}", e);
            }
        }
    }

    async fn poll_all_clusters(&self) -> Result<(), String> {
        let active_clusters = self.cluster_repo.find_all_active().await?;
        
        for cluster in active_clusters {
            if let (Some(log_group), Some(log_stream)) = (&cluster.log_group, &cluster.log_stream) {
                info!("Polling logs for cluster: {} (log_group: {})", cluster.name, log_group);
                
                let start_time = cluster.last_event_timestamp
                    .map(|ts| DateTime::<Utc>::from_utc(ts, Utc))
                    .unwrap_or_else(|| Utc::now() - Duration::hours(1));
                
                let end_time = Utc::now();
                
                if let Err(e) = self.poll_cluster_logs(&cluster, log_group, log_stream, start_time, end_time).await {
                    error!("Failed to poll logs for cluster {}: {}", cluster.name, e);
                }
            } else {
                warn!("Cluster {} has no log group/stream configured", cluster.name);
            }
        }
        
        Ok(())
    }

    async fn poll_cluster_logs(
        &self,
        cluster: &crate::models::aurora_cluster::AuroraCluster,
        log_group: &str,
        log_stream: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<(), String> {
        let logs_client = self.aws_service.get_cloudwatch_logs_client(&cluster.region).await;
        
        let mut next_token: Option<String> = None;
        let mut latest_timestamp: Option<i64> = None;
        
        loop {
            let mut request = logs_client.get_log_events()
                .log_group_name(log_group)
                .log_stream_name(log_stream)
                .start_time(start_time.timestamp_millis())
                .end_time(end_time.timestamp_millis());
            
            if let Some(token) = next_token {
                request = request.next_token(token);
            }
            
            let response = request.send().await
                .map_err(|e| format!("AWS Error fetching logs: {}", e))?;
            
            if let Some(events) = response.events() {
                if !events.is_empty() {
                    let log_data: Vec<String> = events.iter()
                        .filter_map(|e| e.message().map(|m| m.to_string()))
                        .collect();
                    
                    // Ingest the logs
                    self.ingestion_service.ingest_logs(cluster.id, &log_data, &cluster.engine).await?;
                    
                    // Update latest timestamp
                    if let Some(last_event) = events.last() {
                        latest_timestamp = Some(last_event.timestamp().unwrap_or(0));
                    }
                }
            }
            
            next_token = response.next_forward_token().map(|s| s.to_string());
            
            // If no more logs or we hit the same token (end of stream for now)
            if next_token.is_none() || response.events().map(|e| e.is_empty()).unwrap_or(true) {
                break;
            }
        }
        
        // Update checkpoint in DB
        if let Some(ts) = latest_timestamp {
            let naive_ts = DateTime::<Utc>::from_utc(chrono::NaiveDateTime::from_timestamp(ts / 1000, (ts % 1000) as u32 * 1000000), Utc).naive_utc();
            self.cluster_repo.update_checkpoint(cluster.id, naive_ts).await?;
        }
        
        Ok(())
    }
}
