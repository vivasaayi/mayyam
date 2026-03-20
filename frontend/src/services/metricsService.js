import api from './api';

const metricsService = {
  /**
   * Get metrics statistics with optional filtering
   */
  getMetricsStats: async (filters = {}) => {
    const params = new URLSearchParams();
    if (filters.experiment_id) params.append('experiment_id', filters.experiment_id);
    if (filters.resource_type) params.append('resource_type', filters.resource_type);
    if (filters.impact_severity) params.append('impact_severity', filters.impact_severity);
    if (filters.start_date) params.append('start_date', filters.start_date);
    if (filters.end_date) params.append('end_date', filters.end_date);

    const response = await api.get(`/chaos/metrics/stats?${params.toString()}`);
    return response.data;
  },

  /**
   * Get metrics summary for a specific experiment
   */
  getExperimentMetrics: async (experimentId) => {
    const response = await api.get(`/chaos/metrics/experiments/${experimentId}`);
    return response.data;
  },

  /**
   * Get metrics summary for a specific resource type
   */
  getResourceTypeMetrics: async (resourceType) => {
    const response = await api.get(`/chaos/metrics/resource-types/${resourceType}`);
    return response.data;
  },

  /**
   * Format metrics for display
   */
  formatMetrics: (metrics) => {
    return {
      totalExperiments: metrics.total_experiments || 0,
      successfulExperiments: metrics.successful_experiments || 0,
      failedExperiments: metrics.failed_experiments || 0,
      successRate: (metrics.success_rate_percent || 0).toFixed(2),
      avgExecutionDuration: (metrics.avg_execution_duration_ms || 0).toFixed(0),
      avgRecoveryTime: (metrics.avg_recovery_time_ms || 0).toFixed(0),
      avgRollbackTime: (metrics.avg_rollback_time_ms || 0).toFixed(0),
      rollbackSuccessRate: (metrics.rollback_success_rate_percent || 0).toFixed(2),
      mostImpactedResourceType: metrics.most_impacted_resource_type || 'N/A',
      avgImpactSeverity: metrics.avg_impact_severity || 'unknown',
    };
  },
};

export default metricsService;
