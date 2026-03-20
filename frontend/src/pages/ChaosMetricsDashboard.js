import React, { useState, useEffect } from 'react';
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CAlert,
  CSpinner,
  CForm,
  CFormInput,
  CFormSelect,
  CButton,
  CProgress,
} from '@coreui/react';
import CIcon from '@coreui/icons-react';
import { cilReload } from '@coreui/icons';
import metricsService from '../services/metricsService';
import { LineChart, Line, BarChart, Bar, PieChart, Pie, Cell, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';

const ChaosMetricsDashboard = () => {
  const [metrics, setMetrics] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [filters, setFilters] = useState({
    experiment_id: '',
    resource_type: '',
    impact_severity: '',
    start_date: '',
    end_date: '',
  });

  const COLORS = ['#51cf66', '#ff922b', '#ff6b6b', '#748ffc'];

  const fetchMetrics = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await metricsService.getMetricsStats(filters);
      setMetrics(metricsService.formatMetrics(result));
    } catch (err) {
      setError(err.response?.data?.message || 'Failed to fetch metrics');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchMetrics();
  }, [filters]);

  const handleFilterChange = (field, value) => {
    setFilters(prev => ({
      ...prev,
      [field]: value,
    }));
  };

  const StatCard = ({ title, value, unit = '', color = 'primary' }) => (
    <CCard className="mb-3">
      <CCardBody>
        <div className="text-muted small">{title}</div>
        <div className={`h4 mb-0 text-${color}`}>{value}{unit}</div>
      </CCardBody>
    </CCard>
  );

  const statusData = metrics ? [
    { name: 'Successful', value: metrics.successfulExperiments, fill: '#51cf66' },
    { name: 'Failed', value: metrics.failedExperiments, fill: '#ff6b6b' },
  ] : [];

  const durationData = metrics ? [
    { category: 'Execution', duration: parseFloat(metrics.avgExecutionDuration) },
    { category: 'Recovery', duration: parseFloat(metrics.avgRecoveryTime) },
    { category: 'Rollback', duration: parseFloat(metrics.avgRollbackTime) },
  ] : [];

  return (
    <CRow className="mb-3">
      <CCol xs={12}>
        {/* Filters */}
        <CCard className="mb-3">
          <CCardHeader>
            <div className="d-flex justify-content-between align-items-center">
              <span>Metrics Dashboard</span>
              <CButton color="primary" size="sm" onClick={fetchMetrics} disabled={loading}>
                <CIcon icon={cilReload} className="me-2" />
                Refresh
              </CButton>
            </div>
          </CCardHeader>
          <CCardBody>
            <CForm>
              <CRow>
                <CCol md={6} lg={3} className="mb-3">
                  <label className="form-label small">Experiment</label>
                  <CFormInput
                    size="sm"
                    type="text"
                    placeholder="Filter by experiment ID..."
                    value={filters.experiment_id}
                    onChange={(e) => handleFilterChange('experiment_id', e.target.value)}
                  />
                </CCol>

                <CCol md={6} lg={3} className="mb-3">
                  <label className="form-label small">Resource Type</label>
                  <CFormSelect
                    size="sm"
                    value={filters.resource_type}
                    onChange={(e) => handleFilterChange('resource_type', e.target.value)}
                  >
                    <option value="">All Types</option>
                    <option value="EC2">EC2</option>
                    <option value="RDS">RDS</option>
                    <option value="Lambda">Lambda</option>
                    <option value="ECS">ECS</option>
                    <option value="ElastiCache">ElastiCache</option>
                    <option value="DynamoDB">DynamoDB</option>
                    <option value="S3">S3</option>
                    <option value="ALB">ALB</option>
                    <option value="SQS">SQS</option>
                    <option value="EKS">EKS</option>
                  </CFormSelect>
                </CCol>

                <CCol md={6} lg={3} className="mb-3">
                  <label className="form-label small">Impact Severity</label>
                  <CFormSelect
                    size="sm"
                    value={filters.impact_severity}
                    onChange={(e) => handleFilterChange('impact_severity', e.target.value)}
                  >
                    <option value="">All Severities</option>
                    <option value="low">Low</option>
                    <option value="medium">Medium</option>
                    <option value="high">High</option>
                    <option value="critical">Critical</option>
                  </CFormSelect>
                </CCol>

                <CCol md={6} lg={3} className="mb-3">
                  <label className="form-label small">Date Range</label>
                  <CFormInput
                    size="sm"
                    type="date"
                    value={filters.start_date}
                    onChange={(e) => handleFilterChange('start_date', e.target.value)}
                    placeholder="Start date"
                  />
                </CCol>
              </CRow>
            </CForm>
          </CCardBody>
        </CCard>

        {error && <CAlert color="danger" className="mb-3" dismissible onClose={() => setError(null)}>{error}</CAlert>}

        {loading ? (
          <div className="text-center py-4">
            <CSpinner color="primary" />
          </div>
        ) : metrics ? (
          <>
            {/* Key Metrics Cards */}
            <CRow className="mb-3">
              <CCol md={6} lg={3}>
                <StatCard title="Total Experiments" value={metrics.totalExperiments} color="primary" />
              </CCol>
              <CCol md={6} lg={3}>
                <StatCard title="Success Rate" value={metrics.successRate} unit="%" color="success" />
              </CCol>
              <CCol md={6} lg={3}>
                <StatCard title="Avg Execution Time" value={metrics.avgExecutionDuration} unit="ms" color="info" />
              </CCol>
              <CCol md={6} lg={3}>
                <StatCard title="Rollback Success Rate" value={metrics.rollbackSuccessRate} unit="%" color="warning" />
              </CCol>
            </CRow>

            {/* Detailed Metrics */}
            <CRow className="mb-3">
              <CCol md={6} lg={3}>
                <CCard className="mb-3">
                  <CCardBody>
                    <div className="text-muted small">Successful Experiments</div>
                    <div className="h4 mb-0 text-success">{metrics.successfulExperiments}</div>
                    <CProgress value={metrics.totalExperiments > 0 ? (metrics.successfulExperiments / metrics.totalExperiments) * 100 : 0} color="success" className="mt-2" size="sm" />
                  </CCardBody>
                </CCard>
              </CCol>

              <CCol md={6} lg={3}>
                <CCard className="mb-3">
                  <CCardBody>
                    <div className="text-muted small">Failed Experiments</div>
                    <div className="h4 mb-0 text-danger">{metrics.failedExperiments}</div>
                    <CProgress value={metrics.totalExperiments > 0 ? (metrics.failedExperiments / metrics.totalExperiments) * 100 : 0} color="danger" className="mt-2" size="sm" />
                  </CCardBody>
                </CCard>
              </CCol>

              <CCol md={6} lg={3}>
                <CCard className="mb-3">
                  <CCardBody>
                    <div className="text-muted small">Avg Recovery Time</div>
                    <div className="h4 mb-0 text-info">{metrics.avgRecoveryTime}</div>
                    <small className="text-muted">ms</small>
                  </CCardBody>
                </CCard>
              </CCol>

              <CCol md={6} lg={3}>
                <CCard className="mb-3">
                  <CCardBody>
                    <div className="text-muted small">Most Impacted Resource</div>
                    <div className="h5 mb-0">{metrics.mostImpactedResourceType}</div>
                    <small className="text-muted">{metrics.avgImpactSeverity}</small>
                  </CCardBody>
                </CCard>
              </CCol>
            </CRow>

            {/* Charts */}
            <CRow>
              {/* Success vs Failure Pie Chart */}
              <CCol lg={6} className="mb-3">
                <CCard>
                  <CCardHeader>Success vs Failure Distribution</CCardHeader>
                  <CCardBody>
                    {statusData.length > 0 ? (
                      <ResponsiveContainer width="100%" height={300}>
                        <PieChart>
                          <Pie
                            data={statusData}
                            cx="50%"
                            cy="50%"
                            labelLine={false}
                            label={({ name, value }) => `${name}: ${value}`}
                            outerRadius={100}
                            fill="#8884d8"
                            dataKey="value"
                          >
                            {statusData.map((entry, index) => (
                              <Cell key={`cell-${index}`} fill={entry.fill} />
                            ))}
                          </Pie>
                          <Tooltip />
                        </PieChart>
                      </ResponsiveContainer>
                    ) : (
                      <CAlert color="info">No data available</CAlert>
                    )}
                  </CCardBody>
                </CCard>
              </CCol>

              {/* Duration Comparison Bar Chart */}
              <CCol lg={6} className="mb-3">
                <CCard>
                  <CCardHeader>Average Duration Comparison (ms)</CCardHeader>
                  <CCardBody>
                    {durationData.length > 0 && durationData.some(d => d.duration > 0) ? (
                      <ResponsiveContainer width="100%" height={300}>
                        <BarChart data={durationData}>
                          <CartesianGrid strokeDasharray="3 3" />
                          <XAxis dataKey="category" />
                          <YAxis />
                          <Tooltip />
                          <Bar dataKey="duration" fill="#8884d8" />
                        </BarChart>
                      </ResponsiveContainer>
                    ) : (
                      <CAlert color="info">No duration data available</CAlert>
                    )}
                  </CCardBody>
                </CCard>
              </CCol>
            </CRow>

            {/* Additional Metrics */}
            <CRow>
              <CCol lg={12} className="mb-3">
                <CCard>
                  <CCardHeader>Metrics Summary</CCardHeader>
                  <CCardBody>
                    <CRow>
                      <CCol md={6}>
                        <p className="mb-2">
                          <strong>Total Experiments:</strong> {metrics.totalExperiments}
                        </p>
                        <p className="mb-2">
                          <strong>Success Rate:</strong> {metrics.successRate}%
                        </p>
                        <p className="mb-2">
                          <strong>Avg Execution Duration:</strong> {metrics.avgExecutionDuration}ms
                        </p>
                        <p className="mb-2">
                          <strong>Avg Recovery Time:</strong> {metrics.avgRecoveryTime}ms
                        </p>
                      </CCol>
                      <CCol md={6}>
                        <p className="mb-2">
                          <strong>Avg Rollback Time:</strong> {metrics.avgRollbackTime}ms
                        </p>
                        <p className="mb-2">
                          <strong>Rollback Success Rate:</strong> {metrics.rollbackSuccessRate}%
                        </p>
                        <p className="mb-2">
                          <strong>Most Impacted Resource Type:</strong> {metrics.mostImpactedResourceType}
                        </p>
                        <p className="mb-2">
                          <strong>Average Impact Severity:</strong> {metrics.avgImpactSeverity}
                        </p>
                      </CCol>
                    </CRow>
                  </CCardBody>
                </CCard>
              </CCol>
            </CRow>
          </>
        ) : (
          <CAlert color="info">No metrics available</CAlert>
        )}
      </CCol>
    </CRow>
  );
};

export default ChaosMetricsDashboard;
