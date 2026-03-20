import api from './api';

const auditService = {
  /**
   * List audit logs with filtering and pagination
   */
  listAuditLogs: async (filters = {}) => {
    const params = new URLSearchParams();
    if (filters.experiment_id) params.append('experiment_id', filters.experiment_id);
    if (filters.run_id) params.append('run_id', filters.run_id);
    if (filters.action) params.append('action', filters.action);
    if (filters.user_id) params.append('user_id', filters.user_id);
    if (filters.resource_id) params.append('resource_id', filters.resource_id);
    if (filters.triggered_by) params.append('triggered_by', filters.triggered_by);
    if (filters.start_date) params.append('start_date', filters.start_date);
    if (filters.end_date) params.append('end_date', filters.end_date);
    if (filters.page) params.append('page', filters.page);
    if (filters.page_size) params.append('page_size', filters.page_size || 50);

    const response = await api.get(`/chaos/audit/logs?${params.toString()}`);
    return response.data;
  },

  /**
   * Get audit trail for a specific experiment
   */
  getExperimentAuditTrail: async (experimentId) => {
    const response = await api.get(`/chaos/audit/experiments/${experimentId}`);
    return response.data;
  },

  /**
   * Get audit trail for a specific run
   */
  getRunAuditTrail: async (runId) => {
    const response = await api.get(`/chaos/audit/runs/${runId}`);
    return response.data;
  },

  /**
   * Get user activity history
   */
  getUserActivity: async (userId) => {
    const response = await api.get(`/chaos/audit/users/${userId}`);
    return response.data;
  },

  /**
   * Export audit logs as CSV
   */
  exportAuditLogs: async (filters = {}) => {
    const logs = await auditService.listAuditLogs({ ...filters, page_size: 10000 });

    const headers = ['ID', 'Timestamp', 'Action', 'User ID', 'Triggered By', 'Resource ID', 'Status Before', 'Status After', 'IP Address'];
    const rows = logs.logs.map(log => [
      log.id,
      new Date(log.created_at).toLocaleString(),
      log.action,
      log.user_id || '-',
      log.triggered_by || '-',
      log.resource_id || '-',
      log.status_before || '-',
      log.status_after || '-',
      log.ip_address || '-',
    ]);

    const csv = [headers, ...rows].map(row =>
      row.map(cell => `"${String(cell).replace(/"/g, '""')}"`).join(',')
    ).join('\n');

    const blob = new Blob([csv], { type: 'text/csv' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `audit-logs-${new Date().toISOString().split('T')[0]}.csv`;
    a.click();
    window.URL.revokeObjectURL(url);
  },
};

export default auditService;
