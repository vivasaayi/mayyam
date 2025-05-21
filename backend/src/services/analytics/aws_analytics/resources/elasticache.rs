use crate::errors::AppError;
use crate::models::aws_resource;
use crate::services::aws::aws_types::cloud_watch;
use crate::services::analytics::aws_analytics::models::resource_workflows::ResourceAnalysisWorkflow;
use crate::services::analytics::aws_analytics::metrics::MetricsAnalyzer;

pub struct ElastiCacheAnalyzer;

impl ElastiCacheAnalyzer {
    pub async fn analyze_elasticache_cluster(
        _resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# ElastiCache Cluster Performance Analysis\n\n");

                // Cache hit rate analysis
                if let Some(hits_metric) = MetricsAnalyzer::find_metric(metrics, "CacheHits") {
                    if let Some(misses_metric) = MetricsAnalyzer::find_metric(metrics, "CacheMisses") {
                        let (hits_avg, _) = MetricsAnalyzer::calculate_statistics(&hits_metric.datapoints);
                        let (misses_avg, _) = MetricsAnalyzer::calculate_statistics(&misses_metric.datapoints);
                        let hit_rate = hits_avg / (hits_avg + misses_avg) * 100.0;
                        analysis.push_str(&format!(
                            "## Cache Hit Rate\n- Average: {:.2}%\n\n",
                            hit_rate
                        ));
                        
                        // Add recommendations based on hit rate
                        if hit_rate < 80.0 {
                            analysis.push_str("⚠️ **Low Cache Hit Rate Detected**\n");
                            analysis.push_str("Consider:\n");
                            analysis.push_str("1. Reviewing cache key design\n");
                            analysis.push_str("2. Increasing TTL for frequently accessed items\n");
                            analysis.push_str("3. Implementing cache warming strategies\n\n");
                        }
                    }
                }
                
                // CPU utilization analysis
                if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                    analysis.push_str(&format!(
                        "## CPU Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));
                    
                    if max > 80.0 {
                        analysis.push_str("⚠️ **High CPU Usage Detected**\n");
                        analysis.push_str("Consider scaling up the cache nodes or adding more nodes to the cluster.\n\n");
                    }
                }
                
                // Evictions analysis
                if let Some(evictions_metric) = MetricsAnalyzer::find_metric(metrics, "Evictions") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&evictions_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Cache Evictions\n- Average: {:.2} evictions/second\n- Peak: {:.2} evictions/second\n\n",
                        avg, max
                    ));
                    
                    if max > 0.0 {
                        analysis.push_str("⚠️ **Cache Evictions Detected**\n");
                        analysis.push_str("Consider increasing the memory of your cache nodes or adding more nodes.\n\n");
                    }
                }
            },
            ResourceAnalysisWorkflow::Memory => {
                analysis.push_str("# ElastiCache Memory Analysis\n\n");
                
                // Memory usage analysis
                if let Some(memory_metric) = MetricsAnalyzer::find_metric(metrics, "DatabaseMemoryUsagePercentage") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&memory_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Memory Usage\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));
                    
                    if max > 80.0 {
                        analysis.push_str("⚠️ **High Memory Usage Detected**\n");
                        analysis.push_str("Consider:\n");
                        analysis.push_str("1. Increasing the memory of your cache nodes\n");
                        analysis.push_str("2. Adding more nodes to the cluster\n");
                        analysis.push_str("3. Implementing a more aggressive eviction policy\n\n");
                    }
                }
                
                // Swap usage analysis
                if let Some(swap_metric) = MetricsAnalyzer::find_metric(metrics, "SwapUsage") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&swap_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Swap Usage\n- Average: {:.2} bytes\n- Peak: {:.2} bytes\n\n",
                        avg, max
                    ));
                    
                    if max > 0.0 {
                        analysis.push_str("⚠️ **Swap Usage Detected**\n");
                        analysis.push_str("This indicates memory pressure. Consider increasing the memory of your cache nodes.\n\n");
                    }
                }
            },
            _ => return Err(AppError::BadRequest(
                "Unsupported workflow type for ElastiCache cluster".to_string()
            )),
        }

        Ok(analysis)
    }
}