import React, { useState, useEffect, useRef } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CSpinner,
  CAlert,
  CBadge,
  CProgress,
  CProgressBar,
  CNav,
  CNavItem,
  CNavLink,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CFormSelect,
  CButtonGroup
} from "@coreui/react";
import { CChart } from "@coreui/react-chartjs";
import api from "../../services/api";

const DatabaseMonitoring = ({ connection, performanceMetrics }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [activeTab, setActiveTab] = useState("realtime");
  const [monitoringData, setMonitoringData] = useState(null);
  const [timeRange, setTimeRange] = useState("1h");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [refreshInterval, setRefreshInterval] = useState(30); // seconds
  const intervalRef = useRef(null);

  useEffect(() => {
    if (connection && autoRefresh) {
      fetchMonitoringData();
      intervalRef.current = setInterval(fetchMonitoringData, refreshInterval * 1000);
    }

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [connection, autoRefresh, refreshInterval, timeRange]);

  const fetchMonitoringData = async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await api.get(`/api/databases/${connection.id}/monitoring`, {
        params: { time_range: timeRange }
      });
      setMonitoringData(response.data);
    } catch (err) {
      setError("Failed to load monitoring data: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const toggleAutoRefresh = () => {
    setAutoRefresh(!autoRefresh);
    if (!autoRefresh) {
      fetchMonitoringData();
    }
  };

  const getHealthStatus = (metrics) => {
    if (!metrics) return { status: 'unknown', color: 'secondary' };
    
    const cpuUsage = metrics.cpu_usage || 0;
    const memoryUsage = metrics.memory_usage_percent || 0;
    const bufferHitRatio = metrics.buffer_hit_ratio || 0;
    
    if (cpuUsage > 80 || memoryUsage > 90 || bufferHitRatio < 0.8) {
      return { status: 'critical', color: 'danger' };
    } else if (cpuUsage > 60 || memoryUsage > 75 || bufferHitRatio < 0.9) {
      return { status: 'warning', color: 'warning' };
    } else {
      return { status: 'healthy', color: 'success' };
    }
  };

  const formatUptime = (seconds) => {
    if (!seconds) return 'N/A';
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${days}d ${hours}h ${minutes}m`;
  };

  const generateTimeSeriesData = (dataPoints) => {
    if (!dataPoints || dataPoints.length === 0) {
      return {
        labels: [],
        datasets: [{
          label: 'No Data',
          data: [],
          borderColor: 'rgba(75,192,192,1)',
          backgroundColor: 'rgba(75,192,192,0.2)',
          tension: 0.1
        }]
      };
    }

    return {
      labels: dataPoints.map(point => new Date(point.timestamp).toLocaleTimeString()),
      datasets: [{
        label: 'CPU Usage (%)',
        data: dataPoints.map(point => point.cpu_usage || 0),
        borderColor: 'rgba(255,99,132,1)',
        backgroundColor: 'rgba(255,99,132,0.2)',
        tension: 0.1
      }, {
        label: 'Memory Usage (%)',
        data: dataPoints.map(point => point.memory_usage_percent || 0),
        borderColor: 'rgba(54,162,235,1)',
        backgroundColor: 'rgba(54,162,235,0.2)',
        tension: 0.1
      }, {
        label: 'Connections',
        data: dataPoints.map(point => point.active_connections || 0),
        borderColor: 'rgba(75,192,192,1)',
        backgroundColor: 'rgba(75,192,192,0.2)',
        tension: 0.1,
        yAxisID: 'y1'
      }]
    };
  };

  const chartOptions = {
    responsive: true,
    interaction: {
      mode: 'index',
      intersect: false,
    },
    scales: {
      y: {
        type: 'linear',
        display: true,
        position: 'left',
        max: 100,
        min: 0,
        title: {
          display: true,
          text: 'Percentage (%)'
        }
      },
      y1: {
        type: 'linear',
        display: true,
        position: 'right',
        title: {
          display: true,
          text: 'Connections'
        },
        grid: {
          drawOnChartArea: false,
        },
      },
    },
    plugins: {
      legend: {
        position: 'top',
      },
      title: {
        display: true,
        text: 'Database Performance Metrics'
      }
    }
  };

  if (!connection) {
    return <CAlert color="info">Please select a database connection to view monitoring data.</CAlert>;
  }

  const healthStatus = getHealthStatus(monitoringData?.current_metrics);

  return (
    <div>
      {/* Monitoring Header */}
      <CRow className="mb-4">
        <CCol>
          <CCard>
            <CCardHeader>
              <div className="d-flex justify-content-between align-items-center">
                <div className="d-flex align-items-center">
                  <strong>📊 Database Monitoring</strong>
                  <CBadge color={healthStatus.color} className="ms-2">
                    {healthStatus.status.toUpperCase()}
                  </CBadge>
                </div>
                <div className="d-flex gap-2 align-items-center">
                  <CFormSelect
                    size="sm"
                    value={timeRange}
                    onChange={(e) => setTimeRange(e.target.value)}
                    style={{ width: 'auto' }}
                  >
                    <option value="15m">Last 15 minutes</option>
                    <option value="1h">Last hour</option>
                    <option value="6h">Last 6 hours</option>
                    <option value="24h">Last 24 hours</option>
                    <option value="7d">Last 7 days</option>
                  </CFormSelect>
                  
                  <CButtonGroup>
                    <CButton
                      size="sm"
                      color={autoRefresh ? "success" : "secondary"}
                      variant="outline"
                      onClick={toggleAutoRefresh}
                    >
                      {autoRefresh ? "🔄 Auto" : "⏸️ Manual"}
                    </CButton>
                    <CButton
                      size="sm"
                      color="primary"
                      variant="outline"
                      onClick={fetchMonitoringData}
                      disabled={loading}
                    >
                      {loading ? <CSpinner size="sm" /> : "🔄"}
                    </CButton>
                  </CButtonGroup>
                </div>
              </div>
            </CCardHeader>
            <CCardBody>
              <CNav variant="pills">
                <CNavItem>
                  <CNavLink
                    href="#"
                    active={activeTab === "realtime"}
                    onClick={(e) => { e.preventDefault(); setActiveTab("realtime"); }}
                  >
                    📈 Real-time
                  </CNavLink>
                </CNavItem>
                <CNavItem>
                  <CNavLink
                    href="#"
                    active={activeTab === "performance"}
                    onClick={(e) => { e.preventDefault(); setActiveTab("performance"); }}
                  >
                    🚀 Performance
                  </CNavLink>
                </CNavItem>
                <CNavItem>
                  <CNavLink
                    href="#"
                    active={activeTab === "connections"}
                    onClick={(e) => { e.preventDefault(); setActiveTab("connections"); }}
                  >
                    🔗 Connections
                  </CNavLink>
                </CNavItem>
                <CNavItem>
                  <CNavLink
                    href="#"
                    active={activeTab === "queries"}
                    onClick={(e) => { e.preventDefault(); setActiveTab("queries"); }}
                  >
                    🔍 Queries
                  </CNavLink>
                </CNavItem>
                <CNavItem>
                  <CNavLink
                    href="#"
                    active={activeTab === "alerts"}
                    onClick={(e) => { e.preventDefault(); setActiveTab("alerts"); }}
                  >
                    ⚠️ Alerts
                  </CNavLink>
                </CNavItem>
              </CNav>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      {error && (
        <CAlert color="danger" className="mb-4">{error}</CAlert>
      )}

      {/* Tab Content */}
      {activeTab === "realtime" && (
        <div>
          {/* Key Metrics Cards */}
          <CRow className="mb-4">
            <CCol lg={2} md={4} sm={6}>
              <CCard className="text-center">
                <CCardBody>
                  <div className="fs-4 fw-semibold text-primary">
                    {monitoringData?.current_metrics?.cpu_usage?.toFixed(1) || 0}%
                  </div>
                  <div className="text-medium-emphasis small">CPU Usage</div>
                  <CProgress className="mt-2" height={4}>
                    <CProgressBar 
                      value={monitoringData?.current_metrics?.cpu_usage || 0}
                      color={monitoringData?.current_metrics?.cpu_usage > 80 ? "danger" : "primary"}
                    />
                  </CProgress>
                </CCardBody>
              </CCard>
            </CCol>
            <CCol lg={2} md={4} sm={6}>
              <CCard className="text-center">
                <CCardBody>
                  <div className="fs-4 fw-semibold text-info">
                    {monitoringData?.current_metrics?.memory_usage_percent?.toFixed(1) || 0}%
                  </div>
                  <div className="text-medium-emphasis small">Memory Usage</div>
                  <CProgress className="mt-2" height={4}>
                    <CProgressBar 
                      value={monitoringData?.current_metrics?.memory_usage_percent || 0}
                      color={monitoringData?.current_metrics?.memory_usage_percent > 90 ? "danger" : "info"}
                    />
                  </CProgress>
                </CCardBody>
              </CCard>
            </CCol>
            <CCol lg={2} md={4} sm={6}>
              <CCard className="text-center">
                <CCardBody>
                  <div className="fs-4 fw-semibold text-success">
                    {monitoringData?.current_metrics?.active_connections || 0}
                  </div>
                  <div className="text-medium-emphasis small">Active Connections</div>
                  <div className="mt-2 small text-muted">
                    Max: {monitoringData?.current_metrics?.max_connections || 'N/A'}
                  </div>
                </CCardBody>
              </CCard>
            </CCol>
            <CCol lg={2} md={4} sm={6}>
              <CCard className="text-center">
                <CCardBody>
                  <div className="fs-4 fw-semibold text-warning">
                    {((monitoringData?.current_metrics?.buffer_hit_ratio || 0) * 100).toFixed(1)}%
                  </div>
                  <div className="text-medium-emphasis small">Buffer Hit Ratio</div>
                  <CProgress className="mt-2" height={4}>
                    <CProgressBar 
                      value={(monitoringData?.current_metrics?.buffer_hit_ratio || 0) * 100}
                      color={monitoringData?.current_metrics?.buffer_hit_ratio < 0.9 ? "warning" : "success"}
                    />
                  </CProgress>
                </CCardBody>
              </CCard>
            </CCol>
            <CCol lg={2} md={4} sm={6}>
              <CCard className="text-center">
                <CCardBody>
                  <div className="fs-4 fw-semibold text-danger">
                    {monitoringData?.current_metrics?.slow_queries || 0}
                  </div>
                  <div className="text-medium-emphasis small">Slow Queries</div>
                  <div className="mt-2 small text-muted">
                    Last hour
                  </div>
                </CCardBody>
              </CCard>
            </CCol>
            <CCol lg={2} md={4} sm={6}>
              <CCard className="text-center">
                <CCardBody>
                  <div className="fs-4 fw-semibold text-secondary">
                    {formatUptime(monitoringData?.current_metrics?.uptime_seconds)}
                  </div>
                  <div className="text-medium-emphasis small">Uptime</div>
                </CCardBody>
              </CCard>
            </CCol>
          </CRow>

          {/* Performance Chart */}
          <CRow>
            <CCol>
              <CCard>
                <CCardHeader>
                  <strong>Performance Trends</strong>
                </CCardHeader>
                <CCardBody>
                  {monitoringData?.time_series_data ? (
                    <CChart
                      type="line"
                      data={generateTimeSeriesData(monitoringData.time_series_data)}
                      options={chartOptions}
                      height={300}
                    />
                  ) : (
                    <div className="text-center text-muted p-5">
                      <div className="fs-1 mb-3">📈</div>
                      <div>No time series data available</div>
                      {loading && <CSpinner color="primary" className="mt-3" />}
                    </div>
                  )}
                </CCardBody>
              </CCard>
            </CCol>
          </CRow>
        </div>
      )}

      {activeTab === "performance" && (
        <CRow>
          <CCol lg={6}>
            <CCard className="mb-4">
              <CCardHeader><strong>Query Performance</strong></CCardHeader>
              <CCardBody>
                {performanceMetrics?.query_stats ? (
                  <div>
                    <div className="mb-3">
                      <div className="d-flex justify-content-between">
                        <span>Total Queries</span>
                        <strong>{performanceMetrics.query_stats.total_queries?.toLocaleString()}</strong>
                      </div>
                    </div>
                    <div className="mb-3">
                      <div className="d-flex justify-content-between">
                        <span>Slow Queries</span>
                        <strong className="text-danger">{performanceMetrics.query_stats.slow_queries}</strong>
                      </div>
                    </div>
                    <div className="mb-3">
                      <div className="d-flex justify-content-between">
                        <span>Avg Query Time</span>
                        <strong>{performanceMetrics.query_stats.avg_query_time_ms?.toFixed(2)}ms</strong>
                      </div>
                    </div>
                  </div>
                ) : (
                  <div className="text-center text-muted">
                    No query performance data available
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CCol>
          <CCol lg={6}>
            <CCard className="mb-4">
              <CCardHeader><strong>Storage Metrics</strong></CCardHeader>
              <CCardBody>
                {monitoringData?.current_metrics ? (
                  <div>
                    <div className="mb-3">
                      <div className="d-flex justify-content-between">
                        <span>Database Size</span>
                        <strong>{formatBytes(monitoringData.current_metrics.database_size_bytes || 0)}</strong>
                      </div>
                    </div>
                    <div className="mb-3">
                      <div className="d-flex justify-content-between">
                        <span>Data Size</span>
                        <strong>{formatBytes(monitoringData.current_metrics.data_size_bytes || 0)}</strong>
                      </div>
                    </div>
                    <div className="mb-3">
                      <div className="d-flex justify-content-between">
                        <span>Index Size</span>
                        <strong>{formatBytes(monitoringData.current_metrics.index_size_bytes || 0)}</strong>
                      </div>
                    </div>
                  </div>
                ) : (
                  <div className="text-center text-muted">
                    No storage metrics available
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {activeTab === "connections" && (
        <CRow>
          <CCol>
            <CCard>
              <CCardHeader><strong>Active Connections</strong></CCardHeader>
              <CCardBody>
                {monitoringData?.active_connections ? (
                  <CTable striped hover responsive>
                    <CTableHead>
                      <CTableRow>
                        <CTableHeaderCell>ID</CTableHeaderCell>
                        <CTableHeaderCell>User</CTableHeaderCell>
                        <CTableHeaderCell>Host</CTableHeaderCell>
                        <CTableHeaderCell>Database</CTableHeaderCell>
                        <CTableHeaderCell>Command</CTableHeaderCell>
                        <CTableHeaderCell>Time</CTableHeaderCell>
                        <CTableHeaderCell>State</CTableHeaderCell>
                      </CTableRow>
                    </CTableHead>
                    <CTableBody>
                      {monitoringData.active_connections.map((conn, index) => (
                        <CTableRow key={index}>
                          <CTableDataCell>{conn.id}</CTableDataCell>
                          <CTableDataCell>{conn.user}</CTableDataCell>
                          <CTableDataCell>{conn.host}</CTableDataCell>
                          <CTableDataCell>{conn.database || 'N/A'}</CTableDataCell>
                          <CTableDataCell>
                            <CBadge color={conn.command === 'Sleep' ? 'secondary' : 'primary'}>
                              {conn.command}
                            </CBadge>
                          </CTableDataCell>
                          <CTableDataCell>{conn.time}s</CTableDataCell>
                          <CTableDataCell>{conn.state || 'N/A'}</CTableDataCell>
                        </CTableRow>
                      ))}
                    </CTableBody>
                  </CTable>
                ) : (
                  <div className="text-center text-muted">
                    No active connections data available
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {activeTab === "queries" && (
        <CRow>
          <CCol>
            <CCard>
              <CCardHeader><strong>Recent Slow Queries</strong></CCardHeader>
              <CCardBody>
                {performanceMetrics?.query_stats?.top_slow_queries ? (
                  <CTable striped hover responsive>
                    <CTableHead>
                      <CTableRow>
                        <CTableHeaderCell>Query</CTableHeaderCell>
                        <CTableHeaderCell>Avg Time</CTableHeaderCell>
                        <CTableHeaderCell>Executions</CTableHeaderCell>
                        <CTableHeaderCell>Last Execution</CTableHeaderCell>
                      </CTableRow>
                    </CTableHead>
                    <CTableBody>
                      {performanceMetrics.query_stats.top_slow_queries.map((query, index) => (
                        <CTableRow key={index}>
                          <CTableDataCell>
                            <code className="small">
                              {query.query.substring(0, 100)}
                              {query.query.length > 100 && '...'}
                            </code>
                          </CTableDataCell>
                          <CTableDataCell>
                            <CBadge color="danger">
                              {query.avg_execution_time_ms?.toFixed(2)}ms
                            </CBadge>
                          </CTableDataCell>
                          <CTableDataCell>{query.execution_count}</CTableDataCell>
                          <CTableDataCell>
                            {new Date(query.last_execution).toLocaleString()}
                          </CTableDataCell>
                        </CTableRow>
                      ))}
                    </CTableBody>
                  </CTable>
                ) : (
                  <div className="text-center text-muted">
                    No slow queries data available
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {activeTab === "alerts" && (
        <CRow>
          <CCol>
            <CCard>
              <CCardHeader><strong>Database Alerts</strong></CCardHeader>
              <CCardBody>
                {monitoringData?.alerts ? (
                  <div>
                    {monitoringData.alerts.map((alert, index) => (
                      <CAlert key={index} color={alert.severity} className="mb-2">
                        <div className="d-flex justify-content-between align-items-start">
                          <div>
                            <strong>{alert.title}</strong>
                            <div className="mt-1">{alert.description}</div>
                            <small className="text-muted">
                              {new Date(alert.timestamp).toLocaleString()}
                            </small>
                          </div>
                          <CBadge color={alert.severity}>
                            {alert.severity.toUpperCase()}
                          </CBadge>
                        </div>
                      </CAlert>
                    ))}
                  </div>
                ) : (
                  <div className="text-center text-muted">
                    <div className="fs-1 mb-3">✅</div>
                    <div>No active alerts</div>
                    <small>Your database is running smoothly</small>
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}
    </div>
  );
};

// Utility function
const formatBytes = (bytes) => {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

export default DatabaseMonitoring;
