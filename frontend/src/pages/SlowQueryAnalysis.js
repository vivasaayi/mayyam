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
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane,
  CDropdown,
  CDropdownToggle,
  CDropdownMenu,
  CDropdownItem
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import { CChart } from "@coreui/react-chartjs";
import PageHeader from "../components/layout/PageHeader";
import {
  getSlowQueryEvents,
  getSlowQueryStatistics,
  analyzeSlowQueries,
  getAuroraClusters
} from "../services/api";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";

const SlowQueryAnalysis = () => {
  // State management
  const [clusters, setClusters] = useState([]);
  const [selectedCluster, setSelectedCluster] = useState(null);
  const [slowQueries, setSlowQueries] = useState([]);
  const [statistics, setStatistics] = useState(null);
  const [loading, setLoading] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Filter state
  const [filters, setFilters] = useState({
    start_time: "",
    end_time: "",
    min_execution_time: 1,
    max_execution_time: "",
    limit: 100,
    sort_by: "execution_time",
    sort_order: "desc"
  });

  // Modal state for query details
  const [selectedQuery, setSelectedQuery] = useState(null);
  const [showQueryModal, setShowQueryModal] = useState(false);

  // Active tab
  const [activeTab, setActiveTab] = useState("queries");

  // Grid refs
  const queriesGridRef = useRef();

  // Load clusters on component mount
  useEffect(() => {
    fetchClusters();
  }, []);

  // Load slow queries when cluster is selected
  useEffect(() => {
    if (selectedCluster) {
      fetchSlowQueries();
      fetchStatistics();
    }
  }, [selectedCluster, filters]);

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

  const fetchSlowQueries = async () => {
    if (!selectedCluster) return;

    try {
      setLoading(true);
      const params = {
        cluster_id: selectedCluster.id,
        ...filters
      };
      const response = await getSlowQueryEvents(params);
      setSlowQueries(response || []);
    } catch (err) {
      setError("Failed to fetch slow queries: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const fetchStatistics = async () => {
    if (!selectedCluster) return;

    try {
      const response = await getSlowQueryStatistics(selectedCluster.id, filters);
      setStatistics(response);
    } catch (err) {
      console.error("Failed to fetch statistics:", err);
    }
  };

  const handleAnalyzeQueries = async () => {
    if (!selectedCluster) return;

    try {
      setAnalyzing(true);
      const response = await analyzeSlowQueries(selectedCluster.id, filters);
      setSuccess("Slow query analysis completed");
      await fetchSlowQueries();
      await fetchStatistics();
    } catch (err) {
      setError("Failed to analyze slow queries: " + (err.response?.data?.message || err.message));
    } finally {
      setAnalyzing(false);
    }
  };

  const handleFilterChange = (field, value) => {
    setFilters(prev => ({
      ...prev,
      [field]: value
    }));
  };

  const handleViewQueryDetails = (query) => {
    setSelectedQuery(query);
    setShowQueryModal(true);
  };

  const formatDuration = (seconds) => {
    if (seconds < 1) return `${(seconds * 1000).toFixed(2)}ms`;
    if (seconds < 60) return `${seconds.toFixed(2)}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}m ${remainingSeconds.toFixed(2)}s`;
  };

  const formatTimestamp = (timestamp) => {
    return new Date(timestamp).toLocaleString();
  };

  const getSeverityBadge = (executionTime) => {
    if (executionTime > 30) return <CBadge color="danger">Critical</CBadge>;
    if (executionTime > 10) return <CBadge color="warning">High</CBadge>;
    if (executionTime > 1) return <CBadge color="info">Medium</CBadge>;
    return <CBadge color="secondary">Low</CBadge>;
  };

  const queryColumnDefs = [
    {
      headerName: "Timestamp",
      field: "start_time",
      sortable: true,
      filter: true,
      width: 160,
      valueFormatter: (params) => formatTimestamp(params.value)
    },
    {
      headerName: "Execution Time",
      field: "execution_time",
      sortable: true,
      filter: true,
      width: 140,
      valueFormatter: (params) => formatDuration(params.value),
      cellRenderer: (params) => (
        <div className="d-flex align-items-center">
          {getSeverityBadge(params.value)}
          <span className="ms-2">{formatDuration(params.value)}</span>
        </div>
      )
    },
    {
      headerName: "Lock Time",
      field: "lock_time",
      sortable: true,
      filter: true,
      width: 120,
      valueFormatter: (params) => formatDuration(params.value)
    },
    {
      headerName: "Rows Examined",
      field: "rows_examined",
      sortable: true,
      filter: true,
      width: 130,
      type: "numericColumn"
    },
    {
      headerName: "Rows Sent",
      field: "rows_sent",
      sortable: true,
      filter: true,
      width: 110,
      type: "numericColumn"
    },
    {
      headerName: "SQL Query",
      field: "sql_text",
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
      headerName: "Database",
      field: "db",
      sortable: true,
      filter: true,
      width: 120
    },
    {
      headerName: "User",
      field: "user_host",
      sortable: true,
      filter: true,
      width: 120
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
          onClick={() => handleViewQueryDetails(params.data)}
        >
          Details
        </CButton>
      )
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
        title="Slow Query Analysis"
        breadcrumbs={[
          { label: "Performance Analysis", link: "/performance-analysis" },
          { label: "Slow Query Analysis" }
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

      {/* Cluster Selection and Filters */}
      <CCard className="mb-4">
        <CCardHeader>
          <h5 className="mb-0">Analysis Configuration</h5>
        </CCardHeader>
        <CCardBody>
          <CRow>
            <CCol md={3}>
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
            <CCol md={2}>
              <CFormLabel>Min Execution Time (s)</CFormLabel>
              <CFormInput
                type="number"
                value={filters.min_execution_time}
                onChange={(e) => handleFilterChange('min_execution_time', parseFloat(e.target.value))}
                min="0"
                step="0.1"
              />
            </CCol>
            <CCol md={2}>
              <CFormLabel>Max Execution Time (s)</CFormLabel>
              <CFormInput
                type="number"
                value={filters.max_execution_time}
                onChange={(e) => handleFilterChange('max_execution_time', e.target.value ? parseFloat(e.target.value) : "")}
                min="0"
                step="0.1"
                placeholder="No limit"
              />
            </CCol>
            <CCol md={2}>
              <CFormLabel>Start Time</CFormLabel>
              <CFormInput
                type="datetime-local"
                value={filters.start_time}
                onChange={(e) => handleFilterChange('start_time', e.target.value)}
              />
            </CCol>
            <CCol md={2}>
              <CFormLabel>End Time</CFormLabel>
              <CFormInput
                type="datetime-local"
                value={filters.end_time}
                onChange={(e) => handleFilterChange('end_time', e.target.value)}
              />
            </CCol>
            <CCol md={1} className="d-flex align-items-end">
              <CButton
                color="primary"
                onClick={handleAnalyzeQueries}
                disabled={analyzing || !selectedCluster}
                className="w-100"
              >
                {analyzing ? <CSpinner size="sm" /> : "Analyze"}
              </CButton>
            </CCol>
          </CRow>
        </CCardBody>
      </CCard>

      {/* Statistics Cards */}
      {statistics && (
        <CRow className="mb-4">
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{statistics.total_queries?.toLocaleString() || 0}</h4>
                <p className="text-muted mb-0">Total Slow Queries</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{formatDuration(statistics.avg_execution_time || 0)}</h4>
                <p className="text-muted mb-0">Avg Execution Time</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{formatDuration(statistics.max_execution_time || 0)}</h4>
                <p className="text-muted mb-0">Max Execution Time</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{statistics.unique_queries?.toLocaleString() || 0}</h4>
                <p className="text-muted mb-0">Unique Queries</p>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Charts */}
      {statistics && statistics.execution_time_distribution && (
        <CRow className="mb-4">
          <CCol md={6}>
            <CCard>
              <CCardHeader>
                <h5 className="mb-0">Execution Time Distribution</h5>
              </CCardHeader>
              <CCardBody>
                <div style={{ height: '300px' }}>
                  <CChart
                    type="bar"
                    data={{
                      labels: statistics.execution_time_distribution.map(d => d.range),
                      datasets: [{
                        label: 'Query Count',
                        data: statistics.execution_time_distribution.map(d => d.count),
                        backgroundColor: 'rgba(54, 162, 235, 0.5)',
                        borderColor: 'rgba(54, 162, 235, 1)',
                        borderWidth: 1
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
                <h5 className="mb-0">Top Slow Queries by Frequency</h5>
              </CCardHeader>
              <CCardBody>
                <div style={{ height: '300px' }}>
                  <CChart
                    type="doughnut"
                    data={{
                      labels: statistics.top_queries_by_frequency?.slice(0, 10).map(q => q.sql_text?.substring(0, 50) + '...') || [],
                      datasets: [{
                        data: statistics.top_queries_by_frequency?.slice(0, 10).map(q => q.count) || [],
                        backgroundColor: [
                          '#FF6384', '#36A2EB', '#FFCE56', '#4BC0C0',
                          '#9966FF', '#FF9F40', '#FF6384', '#C9CBCF',
                          '#4BC0C0', '#FF6384'
                        ]
                      }]
                    }}
                    options={{
                      responsive: true,
                      maintainAspectRatio: false,
                      plugins: {
                        legend: {
                          position: 'right',
                        }
                      }
                    }}
                  />
                </div>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Queries Table */}
      <CCard>
        <CCardHeader>
          <div className="d-flex justify-content-between align-items-center">
            <h5 className="mb-0">Slow Query Events</h5>
            <div>
              <span className="text-muted me-2">
                Showing {slowQueries.length} queries
                {selectedCluster && ` for ${selectedCluster.cluster_identifier}`}
              </span>
            </div>
          </div>
        </CCardHeader>
        <CCardBody>
          {loading ? (
            <div className="text-center">
              <CSpinner />
              <div className="mt-2">Loading slow queries...</div>
            </div>
          ) : (
            <div className="ag-theme-alpine" style={{ height: '600px', width: '100%' }}>
              <AgGridReact
                ref={queriesGridRef}
                rowData={slowQueries}
                columnDefs={queryColumnDefs}
                pagination={true}
                paginationPageSize={50}
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
          )}
        </CCardBody>
      </CCard>

      {/* Query Details Modal */}
      <CModal size="xl" visible={showQueryModal} onClose={() => setShowQueryModal(false)}>
        <CModalHeader>
          <CModalTitle>Query Details</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {selectedQuery && (
            <div>
              <CRow className="mb-3">
                <CCol md={6}>
                  <strong>Timestamp:</strong> {formatTimestamp(selectedQuery.start_time)}
                </CCol>
                <CCol md={6}>
                  <strong>Execution Time:</strong> {formatDuration(selectedQuery.execution_time)}
                </CCol>
              </CRow>
              <CRow className="mb-3">
                <CCol md={6}>
                  <strong>Database:</strong> {selectedQuery.db}
                </CCol>
                <CCol md={6}>
                  <strong>User:</strong> {selectedQuery.user_host}
                </CCol>
              </CRow>
              <CRow className="mb-3">
                <CCol md={6}>
                  <strong>Rows Examined:</strong> {selectedQuery.rows_examined}
                </CCol>
                <CCol md={6}>
                  <strong>Rows Sent:</strong> {selectedQuery.rows_sent}
                </CCol>
              </CRow>
              <CRow className="mb-3">
                <CCol md={12}>
                  <strong>SQL Query:</strong>
                  <pre style={{
                    backgroundColor: '#f8f9fa',
                    padding: '10px',
                    borderRadius: '4px',
                    fontSize: '12px',
                    whiteSpace: 'pre-wrap',
                    wordBreak: 'break-all'
                  }}>
                    {selectedQuery.sql_text}
                  </pre>
                </CCol>
              </CRow>
              {selectedQuery.explain_plan && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <strong>EXPLAIN Plan:</strong>
                    <pre style={{
                      backgroundColor: '#f8f9fa',
                      padding: '10px',
                      borderRadius: '4px',
                      fontSize: '12px',
                      whiteSpace: 'pre-wrap'
                    }}>
                      {JSON.stringify(selectedQuery.explain_plan, null, 2)}
                    </pre>
                  </CCol>
                </CRow>
              )}
            </div>
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowQueryModal(false)}>
            Close
          </CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default SlowQueryAnalysis;