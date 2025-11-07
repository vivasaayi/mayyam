import React, { useState, useEffect, useRef } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CForm,
  CFormInput,
  CFormLabel,
  CFormSelect,
  CFormTextarea,
  CAlert,
  CSpinner,
  CBadge,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CModal,
  CModalHeader,
  CModalTitle,
  CModalBody,
  CModalFooter,
  CProgress,
  CProgressBar,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane,
  CListGroup,
  CListGroupItem
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import { CChart } from "@coreui/react-chartjs";
import PageHeader from "../components/layout/PageHeader";
import {
  getPerformanceSnapshots,
  getPerformanceSnapshot,
  createPerformanceSnapshot,
  getPerformanceHealthScore,
  getPerformanceTrends,
  getPerformanceAlerts,
  getAuroraClusters
} from "../services/api";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";

const PerformanceMonitoring = () => {
  // State management
  const [clusters, setClusters] = useState([]);
  const [selectedCluster, setSelectedCluster] = useState(null);
  const [snapshots, setSnapshots] = useState([]);
  const [healthScore, setHealthScore] = useState(null);
  const [trends, setTrends] = useState(null);
  const [alerts, setAlerts] = useState([]);
  const [selectedSnapshot, setSelectedSnapshot] = useState(null);
  const [loading, setLoading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Modal states
  const [showSnapshotModal, setShowSnapshotModal] = useState(false);
  const [showTrendsModal, setShowTrendsModal] = useState(false);
  const [activeTab, setActiveTab] = useState("overview");

  // Create snapshot form
  const [snapshotForm, setSnapshotForm] = useState({
    snapshot_type: "comprehensive",
    include_metrics: true,
    include_alerts: true,
    description: ""
  });

  // Grid refs
  const snapshotsGridRef = useRef();
  const alertsGridRef = useRef();

  // Load clusters on component mount
  useEffect(() => {
    fetchClusters();
  }, []);

  // Load data when cluster is selected
  useEffect(() => {
    if (selectedCluster) {
      fetchSnapshots();
      fetchHealthScore();
      fetchTrends();
      fetchAlerts();
    }
  }, [selectedCluster]);

  const fetchClusters = async () => {
    try {
      const response = await getAuroraClusters();
      setClusters(response || []);
      if (response && response.length > 0 && !selectedCluster) {
        setSelectedCluster(response[0]);
      }
    } catch (err) {
      setError("Failed to fetch Aurora clusters: " + (err.response?.data?.message || err.message));
    }
  };

  const fetchSnapshots = async () => {
    if (!selectedCluster) return;

    try {
      setLoading(true);
      const params = { cluster_id: selectedCluster.id };
      const response = await getPerformanceSnapshots(params);
      setSnapshots(response || []);
    } catch (err) {
      setError("Failed to fetch performance snapshots: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const fetchHealthScore = async () => {
    if (!selectedCluster) return;

    try {
      const response = await getPerformanceHealthScore(selectedCluster.id);
      setHealthScore(response);
    } catch (err) {
      console.error("Failed to fetch health score:", err);
    }
  };

  const fetchTrends = async () => {
    if (!selectedCluster) return;

    try {
      const response = await getPerformanceTrends(selectedCluster.id);
      setTrends(response);
    } catch (err) {
      console.error("Failed to fetch trends:", err);
    }
  };

  const fetchAlerts = async () => {
    if (!selectedCluster) return;

    try {
      const response = await getPerformanceAlerts(selectedCluster.id);
      setAlerts(response || []);
    } catch (err) {
      console.error("Failed to fetch alerts:", err);
    }
  };

  const handleCreateSnapshot = async (e) => {
    e.preventDefault();
    try {
      setCreating(true);
      const formData = {
        ...snapshotForm,
        cluster_id: selectedCluster.id
      };

      await createPerformanceSnapshot(formData);
      setSuccess("Performance snapshot created successfully");
      setShowSnapshotModal(false);
      resetSnapshotForm();
      await fetchSnapshots();
    } catch (err) {
      setError("Failed to create performance snapshot: " + (err.response?.data?.message || err.message));
    } finally {
      setCreating(false);
    }
  };

  const handleViewSnapshot = async (snapshot) => {
    try {
      setLoading(true);
      const response = await getPerformanceSnapshot(snapshot.id);
      setSelectedSnapshot(response);
      setShowTrendsModal(true);
    } catch (err) {
      setError("Failed to fetch snapshot details: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const resetSnapshotForm = () => {
    setSnapshotForm({
      snapshot_type: "comprehensive",
      include_metrics: true,
      include_alerts: true,
      description: ""
    });
  };

  const getHealthBadge = (score) => {
    if (score >= 80) return <CBadge color="success">Excellent</CBadge>;
    if (score >= 60) return <CBadge color="info">Good</CBadge>;
    if (score >= 40) return <CBadge color="warning">Fair</CBadge>;
    return <CBadge color="danger">Poor</CBadge>;
  };

  const getAlertSeverityBadge = (severity) => {
    const colors = {
      'critical': 'danger',
      'high': 'warning',
      'medium': 'info',
      'low': 'secondary'
    };
    return <CBadge color={colors[severity] || 'secondary'}>{severity}</CBadge>;
  };

  const formatTimestamp = (timestamp) => {
    return new Date(timestamp).toLocaleString();
  };

  const formatDuration = (seconds) => {
    if (seconds < 1) return `${(seconds * 1000).toFixed(2)}ms`;
    if (seconds < 60) return `${seconds.toFixed(2)}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}m ${remainingSeconds.toFixed(2)}s`;
  };

  const snapshotColumnDefs = [
    {
      headerName: "Snapshot Type",
      field: "snapshot_type",
      sortable: true,
      filter: true,
      width: 140,
      cellRenderer: (params) => (
        <CBadge color="info">{params.value}</CBadge>
      )
    },
    {
      headerName: "Health Score",
      field: "health_score",
      sortable: true,
      filter: true,
      width: 130,
      type: "numericColumn",
      valueFormatter: (params) => `${params.value?.toFixed(1) || 0}%`,
      cellRenderer: (params) => getHealthBadge(params.value)
    },
    {
      headerName: "Avg Query Time",
      field: "avg_query_time",
      sortable: true,
      filter: true,
      width: 140,
      valueFormatter: (params) => formatDuration(params.value)
    },
    {
      headerName: "Active Connections",
      field: "active_connections",
      sortable: true,
      filter: true,
      width: 150,
      type: "numericColumn"
    },
    {
      headerName: "CPU Usage",
      field: "cpu_usage_percent",
      sortable: true,
      filter: true,
      width: 120,
      type: "numericColumn",
      valueFormatter: (params) => `${params.value?.toFixed(1) || 0}%`
    },
    {
      headerName: "Memory Usage",
      field: "memory_usage_percent",
      sortable: true,
      filter: true,
      width: 140,
      type: "numericColumn",
      valueFormatter: (params) => `${params.value?.toFixed(1) || 0}%`
    },
    {
      headerName: "Alerts Count",
      field: "alerts_count",
      sortable: true,
      filter: true,
      width: 120,
      type: "numericColumn"
    },
    {
      headerName: "Created",
      field: "created_at",
      sortable: true,
      filter: true,
      width: 140,
      valueFormatter: (params) => formatTimestamp(params.value)
    },
    {
      headerName: "Actions",
      field: "actions",
      width: 120,
      cellRenderer: (params) => (
        <CButton
          size="sm"
          color="info"
          variant="outline"
          onClick={() => handleViewSnapshot(params.data)}
        >
          View
        </CButton>
      )
    }
  ];

  const alertColumnDefs = [
    {
      headerName: "Severity",
      field: "severity",
      sortable: true,
      filter: true,
      width: 100,
      cellRenderer: (params) => getAlertSeverityBadge(params.value)
    },
    {
      headerName: "Alert Type",
      field: "alert_type",
      sortable: true,
      filter: true,
      width: 140
    },
    {
      headerName: "Message",
      field: "message",
      sortable: false,
      filter: false,
      width: 300,
      cellRenderer: (params) => (
        <div style={{
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          maxWidth: '280px'
        }}>
          {params.value}
        </div>
      )
    },
    {
      headerName: "Metric Value",
      field: "metric_value",
      sortable: true,
      filter: true,
      width: 120,
      type: "numericColumn"
    },
    {
      headerName: "Threshold",
      field: "threshold_value",
      sortable: true,
      filter: true,
      width: 110,
      type: "numericColumn"
    },
    {
      headerName: "Created",
      field: "created_at",
      sortable: true,
      filter: true,
      width: 140,
      valueFormatter: (params) => formatTimestamp(params.value)
    }
  ];

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        position: 'top',
      }
    }
  };

  return (
    <div>
      <PageHeader
        title="Performance Monitoring"
        breadcrumbs={[
          { label: "Performance Analysis", link: "/performance-analysis" },
          { label: "Performance Monitoring" }
        ]}
      />

      {error && (
        <CAlert color="danger" dismissible onClose={() => setError(null)}>
          {error}
        </CAlert>
      )}

      {success && (
        <CAlert color="success" dismissible onClose={() => setSuccess(null)}>
          {success}
        </CAlert>
      )}

      {/* Cluster Selection and Actions */}
      <CCard className="mb-4">
        <CCardHeader>
          <h5 className="mb-0">Performance Monitoring Dashboard</h5>
        </CCardHeader>
        <CCardBody>
          <CRow>
            <CCol md={4}>
              <CFormLabel>Cluster</CFormLabel>
              <CFormSelect
                value={selectedCluster?.id || ""}
                onChange={(e) => {
                  const cluster = clusters.find(c => c.id === parseInt(e.target.value));
                  setSelectedCluster(cluster);
                }}
              >
                <option value="">Select Cluster</option>
                {clusters.map(cluster => (
                  <option key={cluster.id} value={cluster.id}>
                    {cluster.cluster_identifier}
                  </option>
                ))}
              </CFormSelect>
            </CCol>
            <CCol md={8} className="d-flex align-items-end">
              <CButton
                color="primary"
                className="me-2"
                onClick={() => setShowSnapshotModal(true)}
                disabled={!selectedCluster}
              >
                Create Snapshot
              </CButton>
              <CButton
                color="info"
                onClick={() => {
                  fetchHealthScore();
                  fetchTrends();
                  fetchAlerts();
                }}
                disabled={!selectedCluster}
              >
                Refresh Data
              </CButton>
            </CCol>
          </CRow>
        </CCardBody>
      </CCard>

      {/* Health Score Overview */}
      {healthScore && (
        <CRow className="mb-4">
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{healthScore.overall_score?.toFixed(1) || 0}%</h4>
                <p className="text-muted mb-0">Overall Health</p>
                {getHealthBadge(healthScore.overall_score)}
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{healthScore.query_performance?.toFixed(1) || 0}%</h4>
                <p className="text-muted mb-0">Query Performance</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{healthScore.resource_usage?.toFixed(1) || 0}%</h4>
                <p className="text-muted mb-0">Resource Usage</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{healthScore.connection_health?.toFixed(1) || 0}%</h4>
                <p className="text-muted mb-0">Connection Health</p>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Performance Trends */}
      {trends && (
        <CRow className="mb-4">
          <CCol md={6}>
            <CCard>
              <CCardHeader>
                <h5 className="mb-0">Health Score Trend (Last 24h)</h5>
              </CCardHeader>
              <CCardBody>
                <div style={{ height: '300px' }}>
                  <CChart
                    type="line"
                    data={{
                      labels: trends.health_scores?.map(t => new Date(t.timestamp).toLocaleTimeString()) || [],
                      datasets: [{
                        label: 'Health Score',
                        data: trends.health_scores?.map(t => t.value) || [],
                        borderColor: 'rgba(54, 162, 235, 1)',
                        backgroundColor: 'rgba(54, 162, 235, 0.1)',
                        fill: true
                      }]
                    }}
                    options={chartOptions}
                  />
                </div>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={6}>
            <CCard>
              <CCardHeader>
                <h5 className="mb-0">Active Connections Trend</h5>
              </CCardHeader>
              <CCardBody>
                <div style={{ height: '300px' }}>
                  <CChart
                    type="line"
                    data={{
                      labels: trends.connections?.map(t => new Date(t.timestamp).toLocaleTimeString()) || [],
                      datasets: [{
                        label: 'Active Connections',
                        data: trends.connections?.map(t => t.value) || [],
                        borderColor: 'rgba(75, 192, 192, 1)',
                        backgroundColor: 'rgba(75, 192, 192, 0.1)',
                        fill: true
                      }]
                    }}
                    options={chartOptions}
                  />
                </div>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Tabs for different views */}
      <CCard className="mb-4">
        <CCardHeader>
          <CNav variant="tabs">
            <CNavItem>
              <CNavLink active={activeTab === "overview"} onClick={() => setActiveTab("overview")}>
                Overview
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink active={activeTab === "snapshots"} onClick={() => setActiveTab("snapshots")}>
                Snapshots
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink active={activeTab === "alerts"} onClick={() => setActiveTab("alerts")}>
                Alerts ({alerts.length})
              </CNavLink>
            </CNavItem>
          </CNav>
        </CCardHeader>
        <CCardBody>
          <CTabContent>
            <CTabPane visible={activeTab === "overview"}>
              {healthScore && (
                <div>
                  <h5>Health Score Breakdown</h5>
                  <CRow className="mb-3">
                    <CCol md={6}>
                      <div className="mb-2">
                        <div className="d-flex justify-content-between">
                          <span>Query Performance</span>
                          <span>{healthScore.query_performance?.toFixed(1)}%</span>
                        </div>
                        <CProgress value={healthScore.query_performance || 0} />
                      </div>
                      <div className="mb-2">
                        <div className="d-flex justify-content-between">
                          <span>Resource Usage</span>
                          <span>{healthScore.resource_usage?.toFixed(1)}%</span>
                        </div>
                        <CProgress value={healthScore.resource_usage || 0} />
                      </div>
                      <div className="mb-2">
                        <div className="d-flex justify-content-between">
                          <span>Connection Health</span>
                          <span>{healthScore.connection_health?.toFixed(1)}%</span>
                        </div>
                        <CProgress value={healthScore.connection_health || 0} />
                      </div>
                    </CCol>
                    <CCol md={6}>
                      <h6>Key Metrics</h6>
                      <CListGroup>
                        <CListGroupItem>
                          <strong>Avg Query Time:</strong> {formatDuration(healthScore.avg_query_time)}
                        </CListGroupItem>
                        <CListGroupItem>
                          <strong>Active Connections:</strong> {healthScore.active_connections}
                        </CListGroupItem>
                        <CListGroupItem>
                          <strong>CPU Usage:</strong> {healthScore.cpu_usage_percent?.toFixed(1)}%
                        </CListGroupItem>
                        <CListGroupItem>
                          <strong>Memory Usage:</strong> {healthScore.memory_usage_percent?.toFixed(1)}%
                        </CListGroupItem>
                        <CListGroupItem>
                          <strong>Buffer Hit Ratio:</strong> {healthScore.buffer_hit_ratio?.toFixed(1)}%
                        </CListGroupItem>
                      </CListGroup>
                    </CCol>
                  </CRow>
                </div>
              )}
            </CTabPane>

            <CTabPane visible={activeTab === "snapshots"}>
              <div className="ag-theme-alpine" style={{ height: '500px', width: '100%' }}>
                <AgGridReact
                  ref={snapshotsGridRef}
                  rowData={snapshots}
                  columnDefs={snapshotColumnDefs}
                  pagination={true}
                  paginationPageSize={20}
                  enableSorting={true}
                  enableFilter={true}
                  enableColResize={true}
                  defaultColDef={{
                    resizable: true,
                    sortable: true,
                    filter: true
                  }}
                />
              </div>
            </CTabPane>

            <CTabPane visible={activeTab === "alerts"}>
              <div className="ag-theme-alpine" style={{ height: '500px', width: '100%' }}>
                <AgGridReact
                  ref={alertsGridRef}
                  rowData={alerts}
                  columnDefs={alertColumnDefs}
                  pagination={true}
                  paginationPageSize={20}
                  enableSorting={true}
                  enableFilter={true}
                  enableColResize={true}
                  defaultColDef={{
                    resizable: true,
                    sortable: true,
                    filter: true
                  }}
                />
              </div>
            </CTabPane>
          </CTabContent>
        </CCardBody>
      </CCard>

      {/* Create Snapshot Modal */}
      <CModal visible={showSnapshotModal} onClose={() => setShowSnapshotModal(false)}>
        <CModalHeader>
          <CModalTitle>Create Performance Snapshot</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <CForm onSubmit={handleCreateSnapshot}>
            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel>Snapshot Type</CFormLabel>
                <CFormSelect
                  value={snapshotForm.snapshot_type}
                  onChange={(e) => setSnapshotForm({...snapshotForm, snapshot_type: e.target.value})}
                  required
                >
                  <option value="comprehensive">Comprehensive</option>
                  <option value="quick">Quick</option>
                  <option value="detailed">Detailed</option>
                </CFormSelect>
              </CCol>
            </CRow>

            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel className="form-check-label">
                  <input
                    type="checkbox"
                    className="form-check-input me-2"
                    checked={snapshotForm.include_metrics}
                    onChange={(e) => setSnapshotForm({...snapshotForm, include_metrics: e.target.checked})}
                  />
                  Include Performance Metrics
                </CFormLabel>
              </CCol>
              <CCol md={6}>
                <CFormLabel className="form-check-label">
                  <input
                    type="checkbox"
                    className="form-check-input me-2"
                    checked={snapshotForm.include_alerts}
                    onChange={(e) => setSnapshotForm({...snapshotForm, include_alerts: e.target.checked})}
                  />
                  Include Active Alerts
                </CFormLabel>
              </CCol>
            </CRow>

            <CRow className="mb-3">
              <CCol xs={12}>
                <CFormLabel>Description (Optional)</CFormLabel>
                <CFormTextarea
                  rows={2}
                  value={snapshotForm.description}
                  onChange={(e) => setSnapshotForm({...snapshotForm, description: e.target.value})}
                  placeholder="Optional description for this snapshot"
                />
              </CCol>
            </CRow>
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowSnapshotModal(false)}>
            Cancel
          </CButton>
          <CButton
            color="primary"
            onClick={handleCreateSnapshot}
            disabled={creating}
          >
            {creating ? <CSpinner size="sm" /> : "Create Snapshot"}
          </CButton>
        </CModalFooter>
      </CModal>

      {/* Snapshot Details Modal */}
      <CModal size="xl" visible={showTrendsModal} onClose={() => setShowTrendsModal(false)}>
        <CModalHeader>
          <CModalTitle>Performance Snapshot Details</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {selectedSnapshot && (
            <div>
              <CRow className="mb-3">
                <CCol md={12}>
                  <div style={{
                    backgroundColor: '#f8f9fa',
                    padding: '15px',
                    borderRadius: '4px'
                  }}>
                    <h5>Snapshot Summary</h5>
                    <CRow>
                      <CCol md={3}>
                        <strong>Type:</strong> {selectedSnapshot.snapshot_type}
                      </CCol>
                      <CCol md={3}>
                        <strong>Health Score:</strong> {selectedSnapshot.health_score?.toFixed(1)}%
                      </CCol>
                      <CCol md={3}>
                        <strong>Created:</strong> {formatTimestamp(selectedSnapshot.created_at)}
                      </CCol>
                      <CCol md={3}>
                        <strong>Alerts:</strong> {selectedSnapshot.alerts_count || 0}
                      </CCol>
                    </CRow>
                  </div>
                </CCol>
              </CRow>

              {selectedSnapshot.metrics && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>Performance Metrics</h5>
                    <CRow>
                      {Object.entries(selectedSnapshot.metrics).map(([key, value]) => (
                        <CCol md={4} key={key} className="mb-3">
                          <CCard>
                            <CCardBody className="text-center">
                              <h6>{key.replace(/_/g, ' ').toUpperCase()}</h6>
                              <div>{typeof value === 'number' ? value.toLocaleString() : value}</div>
                            </CCardBody>
                          </CCard>
                        </CCol>
                      ))}
                    </CRow>
                  </CCol>
                </CRow>
              )}

              {selectedSnapshot.alerts && selectedSnapshot.alerts.length > 0 && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>Active Alerts</h5>
                    <CTable striped>
                      <CTableHead>
                        <CTableRow>
                          <CTableHeaderCell>Severity</CTableHeaderCell>
                          <CTableHeaderCell>Type</CTableHeaderCell>
                          <CTableHeaderCell>Message</CTableHeaderCell>
                          <CTableHeaderCell>Value</CTableHeaderCell>
                        </CTableRow>
                      </CTableHead>
                      <CTableBody>
                        {selectedSnapshot.alerts.map((alert, index) => (
                          <CTableRow key={index}>
                            <CTableDataCell>{getAlertSeverityBadge(alert.severity)}</CTableDataCell>
                            <CTableDataCell>{alert.alert_type}</CTableDataCell>
                            <CTableDataCell>{alert.message}</CTableDataCell>
                            <CTableDataCell>{alert.metric_value}</CTableDataCell>
                          </CTableRow>
                        ))}
                      </CTableBody>
                    </CTable>
                  </CCol>
                </CRow>
              )}
            </div>
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowTrendsModal(false)}>
            Close
          </CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default PerformanceMonitoring;