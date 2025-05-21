use crate::services::aws::aws_types::cloud_watch;

pub struct MetricsAnalyzer;

impl MetricsAnalyzer {
    pub fn find_metric<'a>(
        metrics: &'a cloud_watch::CloudWatchMetricsResult,
        name: &str
    ) -> Option<&'a cloud_watch::CloudWatchMetricData> {
        metrics.metrics.iter().find(|m| m.metric_name == name)
    }

    pub fn calculate_statistics(datapoints: &[cloud_watch::CloudWatchDatapoint]) -> (f64, f64) {
        if datapoints.is_empty() {
            return (0.0, 0.0);
        }

        let sum: f64 = datapoints.iter().map(|d| d.value).sum();
        let max: f64 = datapoints.iter().map(|d| d.value).fold(f64::NEG_INFINITY, f64::max);
        let avg = sum / datapoints.len() as f64;

        (avg, max)
    }

    pub fn analyze_network_metrics(
        analysis: &mut String,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) {
        analysis.push_str("## Network Performance\n");

        if let Some(net_in) = Self::find_metric(metrics, "NetworkIn") {
            let (avg, max) = Self::calculate_statistics(&net_in.datapoints);
            analysis.push_str(&format!(
                "Network In:\n- Average: {:.2} MB/s\n- Peak: {:.2} MB/s\n\n",
                avg / (1024.0 * 1024.0),
                max / (1024.0 * 1024.0)
            ));
        }

        if let Some(net_out) = Self::find_metric(metrics, "NetworkOut") {
            let (avg, max) = Self::calculate_statistics(&net_out.datapoints);
            analysis.push_str(&format!(
                "Network Out:\n- Average: {:.2} MB/s\n- Peak: {:.2} MB/s\n\n",
                avg / (1024.0 * 1024.0),
                max / (1024.0 * 1024.0)
            ));
        }
    }
}