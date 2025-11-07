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
  CAccordion,
  CAccordionItem,
  CAccordionHeader,
  CAccordionBody
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import { CChart } from "@coreui/react-chartjs";
import PageHeader from "../components/layout/PageHeader";
import {
  getAiAnalyses,
  getAiAnalysis,
  generateAiAnalysis,
  getAiInsights,
  getAuroraClusters
} from "../services/api";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";

const AiAnalysis = () => {
  // State management
  const [clusters, setClusters] = useState([]);
  const [selectedCluster, setSelectedCluster] = useState(null);
  const [analyses, setAnalyses] = useState([]);
  const [insights, setInsights] = useState(null);
  const [selectedAnalysis, setSelectedAnalysis] = useState(null);
  const [loading, setLoading] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Modal states
  const [showAnalysisModal, setShowAnalysisModal] = useState(false);
  const [showGenerateModal, setShowGenerateModal] = useState(false);

  // Generate analysis form
  const [generateForm, setGenerateForm] = useState({
    analysis_type: "comprehensive",
    focus_areas: [],
    time_range: "24h",
    include_recommendations: true,
    include_code_examples: true
  });

  // Grid refs
  const analysesGridRef = useRef();

  // Load clusters on component mount
  useEffect(() => {
    fetchClusters();
  }, []);

  // Load analyses when cluster is selected
  useEffect(() => {
    if (selectedCluster) {
      fetchAnalyses();
      fetchInsights();
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

  const fetchAnalyses = async () => {
    if (!selectedCluster) return;

    try {
      setLoading(true);
      const params = { cluster_id: selectedCluster.id };
      const response = await getAiAnalyses(params);
      setAnalyses(response || []);
    } catch (err) {
      setError("Failed to fetch AI analyses: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const fetchInsights = async () => {
    if (!selectedCluster) return;

    try {
      const response = await getAiInsights(selectedCluster.id);
      setInsights(response);
    } catch (err) {
      console.error("Failed to fetch AI insights:", err);
    }
  };

  const handleGenerateAnalysis = async (e) => {
    e.preventDefault();
    try {
      setGenerating(true);
      const formData = {
        ...generateForm,
        cluster_id: selectedCluster.id,
        focus_areas: Array.isArray(generateForm.focus_areas)
          ? generateForm.focus_areas
          : generateForm.focus_areas.split(',').map(area => area.trim())
      };

      await generateAiAnalysis(formData);
      setSuccess("AI analysis generation started. Results will be available shortly.");
      setShowGenerateModal(false);
      await fetchAnalyses();
    } catch (err) {
      setError("Failed to generate AI analysis: " + (err.response?.data?.message || err.message));
    } finally {
      setGenerating(false);
    }
  };

  const handleViewAnalysis = async (analysis) => {
    try {
      setLoading(true);
      const response = await getAiAnalysis(analysis.id);
      setSelectedAnalysis(response);
      setShowAnalysisModal(true);
    } catch (err) {
      setError("Failed to fetch analysis details: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const getStatusBadge = (status) => {
    const statusColors = {
      'completed': 'success',
      'running': 'warning',
      'failed': 'danger',
      'pending': 'secondary'
    };
    return <CBadge color={statusColors[status] || 'secondary'}>{status}</CBadge>;
  };

  const getConfidenceBadge = (confidence) => {
    if (confidence >= 80) return <CBadge color="success">High</CBadge>;
    if (confidence >= 60) return <CBadge color="info">Medium</CBadge>;
    if (confidence >= 40) return <CBadge color="warning">Low</CBadge>;
    return <CBadge color="danger">Very Low</CBadge>;
  };

  const formatTimestamp = (timestamp) => {
    return new Date(timestamp).toLocaleString();
  };

  const analysisColumnDefs = [
    {
      headerName: "Analysis Type",
      field: "analysis_type",
      sortable: true,
      filter: true,
      width: 140,
      cellRenderer: (params) => (
        <CBadge color="info">{params.value}</CBadge>
      )
    },
    {
      headerName: "Status",
      field: "status",
      sortable: true,
      filter: true,
      width: 100,
      cellRenderer: (params) => getStatusBadge(params.value)
    },
    {
      headerName: "Confidence",
      field: "confidence_score",
      sortable: true,
      filter: true,
      width: 120,
      type: "numericColumn",
      valueFormatter: (params) => `${params.value?.toFixed(1) || 0}%`,
      cellRenderer: (params) => getConfidenceBadge(params.value)
    },
    {
      headerName: "Focus Areas",
      field: "focus_areas",
      sortable: false,
      filter: false,
      width: 200,
      cellRenderer: (params) => (
        <div>
          {params.value && params.value.map((area, index) => (
            <CBadge key={index} color="secondary" className="me-1 mb-1">
              {area}
            </CBadge>
          ))}
        </div>
      )
    },
    {
      headerName: "Key Findings",
      field: "key_findings_count",
      sortable: true,
      filter: true,
      width: 130,
      type: "numericColumn",
      valueFormatter: (params) => `${params.value || 0} findings`
    },
    {
      headerName: "Recommendations",
      field: "recommendations_count",
      sortable: true,
      filter: true,
      width: 140,
      type: "numericColumn",
      valueFormatter: (params) => `${params.value || 0} recommendations`
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
          onClick={() => handleViewAnalysis(params.data)}
          disabled={params.data.status !== 'completed'}
        >
          View
        </CButton>
      )
    }
  ];

  const analysisTypes = [
    { value: "comprehensive", label: "Comprehensive Analysis" },
    { value: "performance", label: "Performance Analysis" },
    { value: "slow_queries", label: "Slow Query Analysis" },
    { value: "index", label: "Index Optimization" },
    { value: "schema", label: "Schema Analysis" },
    { value: "workload", label: "Workload Analysis" }
  ];

  const focusAreas = [
    "query_performance", "index_usage", "schema_design", "connection_pooling",
    "memory_usage", "disk_io", "locking_contention", "query_patterns"
  ];

  const timeRanges = [
    { value: "1h", label: "Last Hour" },
    { value: "24h", label: "Last 24 Hours" },
    { value: "7d", label: "Last 7 Days" },
    { value: "30d", label: "Last 30 Days" }
  ];

  return (
    <div>
      <PageHeader
        title="AI Analysis"
        breadcrumbs={[
          { label: "Performance Analysis", link: "/performance-analysis" },
          { label: "AI Analysis" }
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
          <h5 className="mb-0">AI Analysis Dashboard</h5>
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
                onClick={() => setShowGenerateModal(true)}
                disabled={!selectedCluster}
              >
                Generate New Analysis
              </CButton>
            </CCol>
          </CRow>
        </CCardBody>
      </CCard>

      {/* Insights Summary */}
      {insights && (
        <CRow className="mb-4">
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{insights.total_analyses || 0}</h4>
                <p className="text-muted mb-0">Total Analyses</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{insights.critical_issues || 0}</h4>
                <p className="text-muted mb-0">Critical Issues</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{insights.implemented_recommendations || 0}</h4>
                <p className="text-muted mb-0">Implemented Fixes</p>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={3}>
            <CCard>
              <CCardBody className="text-center">
                <h4>{insights.avg_confidence?.toFixed(1) || 0}%</h4>
                <p className="text-muted mb-0">Avg Confidence</p>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Key Insights */}
      {insights && insights.key_insights && (
        <CCard className="mb-4">
          <CCardHeader>
            <h5 className="mb-0">Key AI Insights</h5>
          </CCardHeader>
          <CCardBody>
            <CAccordion>
              {insights.key_insights.map((insight, index) => (
                <CAccordionItem key={index}>
                  <CAccordionHeader>
                    <div className="d-flex justify-content-between align-items-center w-100">
                      <span>{insight.title}</span>
                      <div>
                        {getConfidenceBadge(insight.confidence)}
                        <CBadge color={insight.priority === 'high' ? 'danger' :
                                      insight.priority === 'medium' ? 'warning' : 'info'} className="ms-2">
                          {insight.priority}
                        </CBadge>
                      </div>
                    </div>
                  </CAccordionHeader>
                  <CAccordionBody>
                    <p>{insight.description}</p>
                    {insight.metrics && (
                      <div className="mt-2">
                        <strong>Key Metrics:</strong>
                        <ul>
                          {insight.metrics.map((metric, idx) => (
                            <li key={idx}>{metric}</li>
                          ))}
                        </ul>
                      </div>
                    )}
                    {insight.recommendations && (
                      <div className="mt-2">
                        <strong>Recommendations:</strong>
                        <ul>
                          {insight.recommendations.map((rec, idx) => (
                            <li key={idx}>{rec}</li>
                          ))}
                        </ul>
                      </div>
                    )}
                  </CAccordionBody>
                </CAccordionItem>
              ))}
            </CAccordion>
          </CCardBody>
        </CCard>
      )}

      {/* Analyses Table */}
      <CCard>
        <CCardHeader>
          <div className="d-flex justify-content-between align-items-center">
            <h5 className="mb-0">AI Analyses</h5>
            <div>
              <span className="text-muted me-2">
                Showing {analyses.length} analyses
                {selectedCluster && ` for ${selectedCluster.cluster_identifier}`}
              </span>
            </div>
          </div>
        </CCardHeader>
        <CCardBody>
          {loading ? (
            <div className="text-center">
              <CSpinner />
              <div className="mt-2">Loading analyses...</div>
            </div>
          ) : (
            <div className="ag-theme-alpine" style={{ height: '600px', width: '100%' }}>
              <AgGridReact
                ref={analysesGridRef}
                rowData={analyses}
                columnDefs={analysisColumnDefs}
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

      {/* Generate Analysis Modal */}
      <CModal size="lg" visible={showGenerateModal} onClose={() => setShowGenerateModal(false)}>
        <CModalHeader>
          <CModalTitle>Generate AI Analysis</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <CForm onSubmit={handleGenerateAnalysis}>
            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel>Analysis Type</CFormLabel>
                <CFormSelect
                  value={generateForm.analysis_type}
                  onChange={(e) => setGenerateForm({...generateForm, analysis_type: e.target.value})}
                  required
                >
                  {analysisTypes.map(type => (
                    <option key={type.value} value={type.value}>{type.label}</option>
                  ))}
                </CFormSelect>
              </CCol>
              <CCol md={6}>
                <CFormLabel>Time Range</CFormLabel>
                <CFormSelect
                  value={generateForm.time_range}
                  onChange={(e) => setGenerateForm({...generateForm, time_range: e.target.value})}
                >
                  {timeRanges.map(range => (
                    <option key={range.value} value={range.value}>{range.label}</option>
                  ))}
                </CFormSelect>
              </CCol>
            </CRow>

            <CRow className="mb-3">
              <CCol xs={12}>
                <CFormLabel>Focus Areas (comma-separated)</CFormLabel>
                <CFormInput
                  type="text"
                  value={generateForm.focus_areas.join(', ')}
                  onChange={(e) => setGenerateForm({
                    ...generateForm,
                    focus_areas: e.target.value.split(',').map(area => area.trim()).filter(area => area)
                  })}
                  placeholder="query_performance, index_usage, schema_design"
                />
                <small className="text-muted">
                  Available: {focusAreas.join(', ')}
                </small>
              </CCol>
            </CRow>

            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel className="form-check-label">
                  <input
                    type="checkbox"
                    className="form-check-input me-2"
                    checked={generateForm.include_recommendations}
                    onChange={(e) => setGenerateForm({...generateForm, include_recommendations: e.target.checked})}
                  />
                  Include Recommendations
                </CFormLabel>
              </CCol>
              <CCol md={6}>
                <CFormLabel className="form-check-label">
                  <input
                    type="checkbox"
                    className="form-check-input me-2"
                    checked={generateForm.include_code_examples}
                    onChange={(e) => setGenerateForm({...generateForm, include_code_examples: e.target.checked})}
                  />
                  Include Code Examples
                </CFormLabel>
              </CCol>
            </CRow>
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowGenerateModal(false)}>
            Cancel
          </CButton>
          <CButton
            color="primary"
            onClick={handleGenerateAnalysis}
            disabled={generating}
          >
            {generating ? <CSpinner size="sm" /> : "Generate Analysis"}
          </CButton>
        </CModalFooter>
      </CModal>

      {/* Analysis Details Modal */}
      <CModal size="xl" visible={showAnalysisModal} onClose={() => setShowAnalysisModal(false)}>
        <CModalHeader>
          <CModalTitle>AI Analysis Details</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {selectedAnalysis && (
            <div>
              <CRow className="mb-3">
                <CCol md={12}>
                  <div style={{
                    backgroundColor: '#f8f9fa',
                    padding: '15px',
                    borderRadius: '4px'
                  }}>
                    <h5>Analysis Summary</h5>
                    <p><strong>Type:</strong> {selectedAnalysis.analysis_type}</p>
                    <p><strong>Confidence:</strong> {selectedAnalysis.confidence_score?.toFixed(1)}%</p>
                    <p><strong>Generated:</strong> {formatTimestamp(selectedAnalysis.created_at)}</p>
                    <p><strong>Focus Areas:</strong> {selectedAnalysis.focus_areas?.join(', ')}</p>
                  </div>
                </CCol>
              </CRow>

              {selectedAnalysis.key_findings && selectedAnalysis.key_findings.length > 0 && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>Key Findings</h5>
                    {selectedAnalysis.key_findings.map((finding, index) => (
                      <div key={index} style={{
                        backgroundColor: '#fff3cd',
                        padding: '12px',
                        borderRadius: '4px',
                        marginBottom: '8px',
                        borderLeft: '4px solid #ffc107'
                      }}>
                        <h6>{finding.title}</h6>
                        <p>{finding.description}</p>
                        {finding.severity && (
                          <CBadge color={finding.severity === 'critical' ? 'danger' :
                                        finding.severity === 'high' ? 'warning' : 'info'}>
                            {finding.severity}
                          </CBadge>
                        )}
                      </div>
                    ))}
                  </CCol>
                </CRow>
              )}

              {selectedAnalysis.recommendations && selectedAnalysis.recommendations.length > 0 && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>Recommendations</h5>
                    {selectedAnalysis.recommendations.map((rec, index) => (
                      <div key={index} style={{
                        backgroundColor: '#d1ecf1',
                        padding: '12px',
                        borderRadius: '4px',
                        marginBottom: '8px',
                        borderLeft: '4px solid #17a2b8'
                      }}>
                        <div className="d-flex justify-content-between align-items-start">
                          <div>
                            <h6>{rec.title}</h6>
                            <p className="mb-1">{rec.description}</p>
                            {rec.impact && (
                              <small className="text-muted">
                                <strong>Expected Impact:</strong> {rec.impact}
                              </small>
                            )}
                          </div>
                          <CBadge color={rec.priority === 'high' ? 'danger' :
                                        rec.priority === 'medium' ? 'warning' : 'info'}>
                            {rec.priority}
                          </CBadge>
                        </div>
                        {rec.implementation_steps && (
                          <div className="mt-2">
                            <small><strong>Implementation:</strong></small>
                            <ol>
                              {rec.implementation_steps.map((step, idx) => (
                                <li key={idx}><small>{step}</small></li>
                              ))}
                            </ol>
                          </div>
                        )}
                        {rec.code_example && (
                          <div className="mt-2">
                            <small><strong>Code Example:</strong></small>
                            <pre style={{
                              backgroundColor: '#f8f9fa',
                              padding: '8px',
                              borderRadius: '4px',
                              fontSize: '11px',
                              marginTop: '4px'
                            }}>
                              {rec.code_example}
                            </pre>
                          </div>
                        )}
                      </div>
                    ))}
                  </CCol>
                </CRow>
              )}

              {selectedAnalysis.performance_metrics && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>Performance Metrics Analysis</h5>
                    <div style={{
                      backgroundColor: '#f8f9fa',
                      padding: '15px',
                      borderRadius: '4px'
                    }}>
                      <CRow>
                        {Object.entries(selectedAnalysis.performance_metrics).map(([key, value]) => (
                          <CCol md={4} key={key} className="mb-2">
                            <div className="text-center">
                              <strong>{key.replace(/_/g, ' ').toUpperCase()}</strong>
                              <div>{typeof value === 'number' ? value.toLocaleString() : value}</div>
                            </div>
                          </CCol>
                        ))}
                      </CRow>
                    </div>
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

export default AiAnalysis;