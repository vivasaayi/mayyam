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
  CProgress,
  CProgressBar
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import { CChart } from "@coreui/react-chartjs";
import PageHeader from "../components/layout/PageHeader";
import {
  getQueryFingerprints,
  getFingerprintAnalysis,
  getFingerprintPatterns,
  getAuroraClusters
} from "../services/api";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";

const QueryFingerprints = () => {
  // State management
  const [clusters, setClusters] = useState([]);
  const [selectedCluster, setSelectedCluster] = useState(null);
  const [fingerprints, setFingerprints] = useState([]);
  const [patterns, setPatterns] = useState([]);
  const [selectedFingerprint, setSelectedFingerprint] = useState(null);
  const [fingerprintAnalysis, setFingerprintAnalysis] = useState(null);
  const [loading, setLoading] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Modal state
  const [showAnalysisModal, setShowAnalysisModal] = useState(false);

  // Grid refs
  const fingerprintsGridRef = useRef();

  // Load clusters on component mount
  useEffect(() => {
    fetchClusters();
  }, []);

  // Load fingerprints when cluster is selected
  useEffect(() => {
    if (selectedCluster) {
      fetchFingerprints();
      fetchPatterns();
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

  const fetchFingerprints = async () => {
    if (!selectedCluster) return;

    try {
      setLoading(true);
      const params = { cluster_id: selectedCluster.id };
      const response = await getQueryFingerprints(params);
      setFingerprints(response || []);
    } catch (err) {
      setError("Failed to fetch query fingerprints: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const fetchPatterns = async () => {
    if (!selectedCluster) return;

    try {
      const response = await getFingerprintPatterns(selectedCluster.id);
      setPatterns(response || []);
    } catch (err) {
      console.error("Failed to fetch fingerprint patterns:", err);
    }
  };

  const handleAnalyzeFingerprint = async (fingerprint) => {
    try {
      setAnalyzing(true);
      setSelectedFingerprint(fingerprint);
      const response = await getFingerprintAnalysis(fingerprint.id);
      setFingerprintAnalysis(response);
      setShowAnalysisModal(true);
    } catch (err) {
      setError("Failed to analyze fingerprint: " + (err.response?.data?.message || err.message));
    } finally {
      setAnalyzing(false);
    }
  };

  const formatDuration = (seconds) => {
    if (seconds < 1) return `${(seconds * 1000).toFixed(2)}ms`;
    if (seconds < 60) return `${seconds.toFixed(2)}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}m ${remainingSeconds.toFixed(2)}s`;
  };

  const getEfficiencyBadge = (efficiency) => {
    if (efficiency >= 80) return <CBadge color="success">Excellent</CBadge>;
    if (efficiency >= 60) return <CBadge color="info">Good</CBadge>;
    if (efficiency >= 40) return <CBadge color="warning">Fair</CBadge>;
    return <CBadge color="danger">Poor</CBadge>;
  };

  const getPatternTypeBadge = (patternType) => {
    const colors = {
      'SELECT': 'primary',
      'INSERT': 'success',
      'UPDATE': 'warning',
      'DELETE': 'danger',
      'DDL': 'info',
      'OTHER': 'secondary'
    };
    return <CBadge color={colors[patternType] || 'secondary'}>{patternType}</CBadge>;
  };

  const fingerprintColumnDefs = [
    {
      headerName: "Pattern",
      field: "query_pattern",
      sortable: false,
      filter: false,
      width: 300,
      cellRenderer: (params) => (
        <div style={{
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          maxWidth: '280px',
          fontFamily: 'monospace',
          fontSize: '12px'
        }}>
          {params.value}
        </div>
      )
    },
    {
      headerName: "Type",
      field: "query_type",
      sortable: true,
      filter: true,
      width: 100,
      cellRenderer: (params) => getPatternTypeBadge(params.value)
    },
    {
      headerName: "Frequency",
      field: "frequency",
      sortable: true,
      filter: true,
      width: 120,
      type: "numericColumn",
      valueFormatter: (params) => params.value?.toLocaleString() || 0
    },
    {
      headerName: "Avg Execution Time",
      field: "avg_execution_time",
      sortable: true,
      filter: true,
      width: 160,
      valueFormatter: (params) => formatDuration(params.value),
      cellRenderer: (params) => (
        <div className="d-flex align-items-center">
          {getEfficiencyBadge(params.data?.efficiency_score)}
          <span className="ms-2">{formatDuration(params.value)}</span>
        </div>
      )
    },
    {
      headerName: "Max Execution Time",
      field: "max_execution_time",
      sortable: true,
      filter: true,
      width: 160,
      valueFormatter: (params) => formatDuration(params.value)
    },
    {
      headerName: "Total Executions",
      field: "total_executions",
      sortable: true,
      filter: true,
      width: 140,
      type: "numericColumn",
      valueFormatter: (params) => params.value?.toLocaleString() || 0
    },
    {
      headerName: "Efficiency Score",
      field: "efficiency_score",
      sortable: true,
      filter: true,
      width: 140,
      type: "numericColumn",
      valueFormatter: (params) => `${params.value?.toFixed(1) || 0}%`,
      cellRenderer: (params) => (
        <div className="d-flex align-items-center">
          <CProgress className="flex-grow-1 me-2" style={{ height: '8px' }}>
            <CProgressBar value={params.value || 0} />
          </CProgress>
          <span style={{ minWidth: '35px' }}>{params.value?.toFixed(1) || 0}%</span>
        </div>
      )
    },
    {
      headerName: "Last Seen",
      field: "last_seen",
      sortable: true,
      filter: true,
      width: 140,
      valueFormatter: (params) => {
        if (!params.value) return '';
        return new Date(params.value).toLocaleDateString();
      }
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
          onClick={() => handleAnalyzeFingerprint(params.data)}
          disabled={analyzing}
        >
          {analyzing && selectedFingerprint?.id === params.data.id ?
            <CSpinner size="sm" /> : "Analyze"}
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
        title="Query Fingerprints"
        breadcrumbs={[
          { label: "Performance Analysis", link: "/performance-analysis" },
          { label: "Query Fingerprints" }
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

      {/* Cluster Selection */}
      <CCard className="mb-4">
        <CCardHeader>
          <h5 className="mb-0">Cluster Selection</h5>
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
          </CRow>
        </CCardBody>
      </CCard>

      {/* Pattern Statistics */}
      {patterns && patterns.length > 0 && (
        <CRow className="mb-4">
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{patterns.length}</h4>
                <p className="text-muted mb-0">Total Patterns</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{patterns.filter(p => p.query_type === 'SELECT').length}</h4>
                <p className="text-muted mb-0">SELECT Patterns</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{patterns.filter(p => p.efficiency_score < 50).length}</h4>
                <p className="text-muted mb-0">Inefficient Patterns</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{patterns.reduce((sum, p) => sum + (p.frequency || 0), 0).toLocaleString()}</h4>
                <p className="text-muted mb-0">Total Executions</p>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Charts */}
      {patterns && patterns.length > 0 && (
        <CRow className="mb-4">
          <CCol md={6}>
            <CCard>
              <CCardHeader>
                <h5 className="mb-0">Query Types Distribution</h5>
              </CCardHeader>
              <CCardBody>
                <div style={{ height: '300px' }}>
                  <CChart
                    type="doughnut"
                    data={{
                      labels: ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'DDL', 'OTHER'],
                      datasets: [{
                        data: [
                          patterns.filter(p => p.query_type === 'SELECT').length,
                          patterns.filter(p => p.query_type === 'INSERT').length,
                          patterns.filter(p => p.query_type === 'UPDATE').length,
                          patterns.filter(p => p.query_type === 'DELETE').length,
                          patterns.filter(p => p.query_type === 'DDL').length,
                          patterns.filter(p => p.query_type === 'OTHER').length
                        ],
                        backgroundColor: [
                          '#007bff', '#28a745', '#ffc107', '#dc3545',
                          '#17a2b8', '#6c757d'
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
          <CCol md={6}>
            <CCard>
              <CCardHeader>
                <h5 className="mb-0">Top Patterns by Frequency</h5>
              </CCardHeader>
              <CCardBody>
                <div style={{ height: '300px' }}>
                  <CChart
                    type="bar"
                    data={{
                      labels: patterns.slice(0, 10).map(p => `Pattern ${p.id}`),
                      datasets: [{
                        label: 'Frequency',
                        data: patterns.slice(0, 10).map(p => p.frequency || 0),
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
        </CRow>
      )}

      {/* Fingerprints Table */}
      <CCard>
        <CCardHeader>
          <div className="d-flex justify-content-between align-items-center">
            <h5 className="mb-0">Query Fingerprints</h5>
            <div>
              <span className="text-muted me-2">
                Showing {fingerprints.length} fingerprints
                {selectedCluster && ` for ${selectedCluster.cluster_identifier}`}
              </span>
            </div>
          </div>
        </CCardHeader>
        <CCardBody>
          {loading ? (
            <div className="text-center">
              <CSpinner />
              <div className="mt-2">Loading fingerprints...</div>
            </div>
          ) : (
            <div className="ag-theme-alpine" style={{ height: '600px', width: '100%' }}>
              <AgGridReact
                ref={fingerprintsGridRef}
                rowData={fingerprints}
                columnDefs={fingerprintColumnDefs}
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

      {/* Analysis Modal */}
      <CModal size="xl" visible={showAnalysisModal} onClose={() => setShowAnalysisModal(false)}>
        <CModalHeader>
          <CModalTitle>Fingerprint Analysis</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {fingerprintAnalysis && (
            <div>
              <CRow className="mb-3">
                <CCol md={12}>
                  <h5>Query Pattern</h5>
                  <pre style={{
                    backgroundColor: '#f8f9fa',
                    padding: '10px',
                    borderRadius: '4px',
                    fontSize: '12px',
                    whiteSpace: 'pre-wrap',
                    wordBreak: 'break-all'
                  }}>
                    {fingerprintAnalysis.query_pattern}
                  </pre>
                </CCol>
              </CRow>

              <CRow className="mb-3">
                <CCol md={3}>
                  <strong>Type:</strong> {getPatternTypeBadge(fingerprintAnalysis.query_type)}
                </CCol>
                <CCol md={3}>
                  <strong>Frequency:</strong> {fingerprintAnalysis.frequency?.toLocaleString()}
                </CCol>
                <CCol md={3}>
                  <strong>Avg Time:</strong> {formatDuration(fingerprintAnalysis.avg_execution_time)}
                </CCol>
                <CCol md={3}>
                  <strong>Efficiency:</strong> {fingerprintAnalysis.efficiency_score?.toFixed(1)}%
                </CCol>
              </CRow>

              {fingerprintAnalysis.ai_insights && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>AI Insights</h5>
                    <div style={{
                      backgroundColor: '#e7f3ff',
                      padding: '15px',
                      borderRadius: '4px',
                      borderLeft: '4px solid #007bff'
                    }}>
                      <h6>Analysis Summary</h6>
                      <p>{fingerprintAnalysis.ai_insights.summary}</p>

                      {fingerprintAnalysis.ai_insights.recommendations && (
                        <div className="mt-3">
                          <h6>Recommendations</h6>
                          <ul>
                            {fingerprintAnalysis.ai_insights.recommendations.map((rec, index) => (
                              <li key={index}>{rec}</li>
                            ))}
                          </ul>
                        </div>
                      )}

                      {fingerprintAnalysis.ai_insights.potential_issues && (
                        <div className="mt-3">
                          <h6>Potential Issues</h6>
                          <ul>
                            {fingerprintAnalysis.ai_insights.potential_issues.map((issue, index) => (
                              <li key={index} style={{ color: '#dc3545' }}>{issue}</li>
                            ))}
                          </ul>
                        </div>
                      )}
                    </div>
                  </CCol>
                </CRow>
              )}

              {fingerprintAnalysis.sample_queries && fingerprintAnalysis.sample_queries.length > 0 && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>Sample Queries</h5>
                    {fingerprintAnalysis.sample_queries.slice(0, 3).map((query, index) => (
                      <div key={index} className="mb-2">
                        <small className="text-muted">Query {index + 1}:</small>
                        <pre style={{
                          backgroundColor: '#f8f9fa',
                          padding: '8px',
                          borderRadius: '4px',
                          fontSize: '11px',
                          whiteSpace: 'pre-wrap',
                          wordBreak: 'break-all'
                        }}>
                          {query}
                        </pre>
                      </div>
                    ))}
                  </CCol>
                </CRow>
              )}
            </div>
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowAnalysisModal(false)}>
            Close
          </CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default QueryFingerprints;