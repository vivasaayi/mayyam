use chrono::{DateTime, Utc};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::{api::ListParams, Api};
use serde::Serialize;
use tracing::{debug, error};

use crate::{
    errors::AppError, models::cluster::KubernetesClusterConfig,
    services::kubernetes::client::ClientFactory,
};

#[derive(Debug, Serialize, Clone)]
pub struct UsageSummary {
    pub count: usize,
    pub cpu_cores: f64,
    pub memory_bytes: f64,
    pub cpu_formatted: String,
    pub memory_formatted: String,
}

impl UsageSummary {
    fn new(count: usize, cpu_cores: f64, memory_bytes: f64) -> Self {
        UsageSummary {
            count,
            cpu_cores,
            memory_bytes,
            cpu_formatted: format_cpu(cpu_cores),
            memory_formatted: format_bytes(memory_bytes),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ContainerMetricSummary {
    pub name: String,
    pub cpu_cores: f64,
    pub memory_bytes: f64,
    pub cpu_formatted: String,
    pub memory_formatted: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PodMetricSummary {
    pub name: String,
    pub namespace: String,
    pub total_cpu_cores: f64,
    pub total_memory_bytes: f64,
    pub cpu_formatted: String,
    pub memory_formatted: String,
    pub containers: Vec<ContainerMetricSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct NodeMetricSummary {
    pub name: String,
    pub cpu_cores: f64,
    pub memory_bytes: f64,
    pub cpu_formatted: String,
    pub memory_formatted: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ClusterMetricsOverview {
    pub timestamp: DateTime<Utc>,
    pub namespace: Option<String>,
    pub metrics_available: bool,
    pub message: Option<String>,
    pub node_totals: UsageSummary,
    pub nodes: Vec<NodeMetricSummary>,
    pub pod_totals: UsageSummary,
    pub pods: Vec<PodMetricSummary>,
}

impl ClusterMetricsOverview {
    fn empty(namespace: Option<String>, message: Option<String>) -> Self {
        ClusterMetricsOverview {
            timestamp: Utc::now(),
            namespace,
            metrics_available: false,
            message,
            node_totals: UsageSummary::new(0, 0.0, 0.0),
            nodes: Vec::new(),
            pod_totals: UsageSummary::new(0, 0.0, 0.0),
            pods: Vec::new(),
        }
    }
}

pub struct MetricsService;

impl MetricsService {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_cluster_metrics(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: Option<&str>,
    ) -> Result<ClusterMetricsOverview, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let namespace_raw = namespace.map(|s| s.to_string());
        let namespace_owned = namespace_raw.as_ref().and_then(|ns| {
            if ns.is_empty() || ns == "all" {
                None
            } else {
                Some(ns.clone())
            }
        });
        let ns_display = namespace_owned.clone().or(namespace_raw.clone());
        debug!(
            target: "mayyam::services::kubernetes::metrics",
            namespace = ?ns_display,
            "Collecting cluster metrics"
        );

        let mut metrics = ClusterMetricsOverview {
            timestamp: Utc::now(),
            namespace: ns_display,
            metrics_available: true,
            message: None,
            node_totals: UsageSummary::new(0, 0.0, 0.0),
            nodes: Vec::new(),
            pod_totals: UsageSummary::new(0, 0.0, 0.0),
            pods: Vec::new(),
        };

        // Collect node metrics
        match self.collect_node_metrics(&client).await {
            Ok(node_summaries) => {
                let total_cpu = node_summaries.iter().map(|n| n.cpu_cores).sum();
                let total_memory = node_summaries.iter().map(|n| n.memory_bytes).sum();
                let count = node_summaries.len();
                metrics.node_totals = UsageSummary::new(count, total_cpu, total_memory);
                metrics.nodes = node_summaries;
            }
            Err(e) => {
                error!(
                    target: "mayyam::services::kubernetes::metrics",
                    error = %e,
                    "Failed to collect node metrics"
                );
                metrics.metrics_available = false;
                metrics.message = Some(format!(
                    "Unable to collect node metrics from metrics.k8s.io API: {}",
                    e
                ));
            }
        }

        // Collect pod metrics
        match self
            .collect_pod_metrics(&client, namespace_owned.as_deref())
            .await
        {
            Ok(pod_summaries) => {
                let total_cpu = pod_summaries.iter().map(|p| p.total_cpu_cores).sum();
                let total_memory = pod_summaries.iter().map(|p| p.total_memory_bytes).sum();
                let count = pod_summaries.len();
                metrics.pod_totals = UsageSummary::new(count, total_cpu, total_memory);
                metrics.pods = pod_summaries;
                if metrics.message.is_none() {
                    metrics.metrics_available = true;
                }
            }
            Err(e) => {
                error!(
                    target: "mayyam::services::kubernetes::metrics",
                    error = %e,
                    "Failed to collect pod metrics"
                );
                metrics.metrics_available = false;
                metrics.message = Some(match metrics.message {
                    Some(existing) => format!("{}; Pod metrics unavailable: {}", existing, e),
                    None => format!(
                        "Unable to collect pod metrics from metrics.k8s.io API: {}",
                        e
                    ),
                });
            }
        }

        if !metrics.metrics_available && metrics.nodes.is_empty() && metrics.pods.is_empty() {
            return Ok(ClusterMetricsOverview::empty(
                metrics.namespace.clone(),
                metrics.message,
            ));
        }

        Ok(metrics)
    }

    async fn collect_node_metrics(
        &self,
        _client: &kube::Client,
    ) -> Result<Vec<NodeMetricSummary>, AppError> {
        // Metrics API not available, return empty
        Ok(vec![])
    }

    async fn collect_pod_metrics(
        &self,
        _client: &kube::Client,
        _namespace: Option<&str>,
    ) -> Result<Vec<PodMetricSummary>, AppError> {
        // Metrics API not available, return empty
        Ok(vec![])
    }
}

fn parse_cpu_quantity(quantity: &Quantity) -> Option<f64> {
    let raw = quantity.0.as_str().trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(stripped) = raw.strip_suffix('n') {
        stripped.parse::<f64>().ok().map(|v| v / 1_000_000_000.0)
    } else if let Some(stripped) = raw.strip_suffix('u') {
        stripped.parse::<f64>().ok().map(|v| v / 1_000_000.0)
    } else if let Some(stripped) = raw.strip_suffix('m') {
        stripped.parse::<f64>().ok().map(|v| v / 1000.0)
    } else {
        raw.parse::<f64>().ok()
    }
}

fn parse_memory_quantity(quantity: &Quantity) -> Option<f64> {
    parse_resource_quantity(quantity.0.as_str())
}

fn parse_resource_quantity(raw: &str) -> Option<f64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    const BINARY_UNITS: [(&str, f64); 6] = [
        ("Ki", 1_024.0),
        ("Mi", 1_048_576.0),
        ("Gi", 1_073_741_824.0),
        ("Ti", 1_099_511_627_776.0),
        ("Pi", 1_125_899_906_842_624.0),
        ("Ei", 1_152_921_504_606_846_976.0),
    ];
    for &(suffix, multiplier) in BINARY_UNITS.iter() {
        if raw.ends_with(suffix) {
            let value = raw.trim_end_matches(suffix).parse::<f64>().ok()?;
            return Some(value * multiplier);
        }
    }

    const DECIMAL_UNITS: [(&str, f64); 6] = [
        ("k", 1_000_f64),
        ("M", 1_000_000_f64),
        ("G", 1_000_000_000_f64),
        ("T", 1_000_000_000_000_f64),
        ("P", 1_000_000_000_000_000_f64),
        ("E", 1_000_000_000_000_000_000_f64),
    ];
    for &(suffix, multiplier) in DECIMAL_UNITS.iter() {
        if raw.ends_with(suffix) {
            let value = raw.trim_end_matches(suffix).parse::<f64>().ok()?;
            return Some(value * multiplier);
        }
    }

    raw.parse::<f64>().ok()
}

fn format_cpu(cores: f64) -> String {
    if cores >= 1.0 {
        format!("{:.2} cores", cores)
    } else {
        format!("{:.0} mCPU", cores * 1000.0)
    }
}

fn format_bytes(bytes: f64) -> String {
    if bytes <= 0.0 {
        return "0 B".to_string();
    }
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut value = bytes;
    let mut unit_index = 0;
    while value >= 1024.0 && unit_index < units.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    format!("{:.2} {}", value, units[unit_index])
}

// Implement Default so service can be easily registered with web::Data
impl Default for MetricsService {
    fn default() -> Self {
        Self::new()
    }
}
