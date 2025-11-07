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
  CFormSelect,
  CInputGroup,
  CInputGroupText
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import { CChart } from "@coreui/react-chartjs";
import PageHeader from "../components/layout/PageHeader";
import {
  getExplainPlans,
  createExplainPlan,
  compareExplainPlans,
  getPlanOptimization,
  getAuroraClusters
} from "../services/api";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";

const ExplainPlans = () => {
  // State management
  const [clusters, setClusters] = useState([]);
  const [selectedCluster, setSelectedCluster] = useState(null);
  const [plans, setPlans] = useState([]);
  const [selectedPlans, setSelectedPlans] = useState([]);
  const [comparisonResult, setComparisonResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Modal states
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showComparisonModal, setShowComparisonModal] = useState(false);
  const [showOptimizationModal, setShowOptimizationModal] = useState(false);
  const [selectedPlanForOptimization, setSelectedPlanForOptimization] = useState(null);
  const [optimizationResult, setOptimizationResult] = useState(null);

  // Create plan form
  const [planForm, setPlanForm] = useState({
    query_text: "",
    database_name: "",
    parameters: ""
  });

  // Grid refs
  const plansGridRef = useRef();

  // Load clusters on component mount
  useEffect(() => {
    fetchClusters();
  }, []);

  // Load plans when cluster is selected
  useEffect(() => {
    if (selectedCluster) {
      fetchPlans();
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

  const fetchPlans = async () => {
    if (!selectedCluster) return;

    try {
      setLoading(true);
      const params = { cluster_id: selectedCluster.id };
      const response = await getExplainPlans(params);
      setPlans(response || []);
    } catch (err) {
      setError("Failed to fetch explain plans: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const resetPlanForm = () => {
    setPlanForm({
      query_text: "",
      database_name: "",
      parameters: ""
    });
  };

  const handleCreatePlan = async (e) => {
    e.preventDefault();
    try {
      setLoading(true);
      const formData = {
        ...planForm,
        cluster_id: selectedCluster.id,
        parameters: planForm.parameters ? JSON.parse(planForm.parameters) : {}
      };

      await createExplainPlan(formData);
      setSuccess("Explain plan created successfully");
      setShowCreateModal(false);
      resetPlanForm();
      await fetchPlans();
    } catch (err) {
      setError("Failed to create explain plan: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleComparePlans = async () => {
    if (selectedPlans.length < 2) {
      setError("Please select at least 2 plans to compare");
      return;
    }

    try {
      setAnalyzing(true);
      const response = await compareExplainPlans(selectedPlans.map(p => p.id));
      setComparisonResult(response);
      setShowComparisonModal(true);
    } catch (err) {
      setError("Failed to compare plans: " + (err.response?.data?.message || err.message));
    } finally {
      setAnalyzing(false);
    }
  };

  const handleGetOptimization = async (plan) => {
    try {
      setAnalyzing(true);
      setSelectedPlanForOptimization(plan);
      const response = await getPlanOptimization(plan.id);
      setOptimizationResult(response);
      setShowOptimizationModal(true);
    } catch (err) {
      setError("Failed to get optimization suggestions: " + (err.response?.data?.message || err.message));
    } finally {
      setAnalyzing(false);
    }
  };

  const formatCost = (cost) => {
    if (cost === null || cost === undefined) return 'N/A';
    return cost.toLocaleString(undefined, { maximumFractionDigits: 2 });
  };

  const getEfficiencyBadge = (efficiency) => {
    if (efficiency >= 80) return <CBadge color="success">Excellent</CBadge>;
    if (efficiency >= 60) return <CBadge color="info">Good</CBadge>;
    if (efficiency >= 40) return <CBadge color="warning">Fair</CBadge>;
    return <CBadge color="danger">Poor</CBadge>;
  };

  const renderPlanTree = (planData, level = 0) => {
    if (!planData) return null;

    return (
      <div style={{ marginLeft: `${level * 20}px`, marginBottom: '8px' }}>
        <div style={{
          padding: '8px',
          border: '1px solid #dee2e6',
          borderRadius: '4px',
          backgroundColor: level === 0 ? '#f8f9fa' : '#ffffff'
        }}>
          <div className="d-flex justify-content-between align-items-center">
            <strong>{planData.operation || 'UNKNOWN'}</strong>
            <small className="text-muted">
              Cost: {formatCost(planData.cost)}
              {planData.rows && ` | Rows: ${planData.rows}`}
            </small>
          </div>
          {planData.table && (
            <div><small>Table: {planData.table}</small></div>
          )}
          {planData.key && (
            <div><small>Key: {planData.key}</small></div>
          )}
          {planData.condition && (
            <div><small>Condition: {planData.condition}</small></div>
          )}
        </div>
        {planData.children && planData.children.map((child, index) =>
          <div key={index}>{renderPlanTree(child, level + 1)}</div>
        )}
      </div>
    );
  };

  const planColumnDefs = [
    {
      headerName: "Query",
      field: "query_text",
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
      headerName: "Database",
      field: "database_name",
      sortable: true,
      filter: true,
      width: 120
    },
    {
      headerName: "Total Cost",
      field: "total_cost",
      sortable: true,
      filter: true,
      width: 120,
      type: "numericColumn",
      valueFormatter: (params) => formatCost(params.value)
    },
    {
      headerName: "Execution Time",
      field: "execution_time_ms",
      sortable: true,
      filter: true,
      width: 140,
      type: "numericColumn",
      valueFormatter: (params) => params.value ? `${params.value.toFixed(2)}ms` : 'N/A'
    },
    {
      headerName: "Efficiency",
      field: "efficiency_score",
      sortable: true,
      filter: true,
      width: 120,
      type: "numericColumn",
      valueFormatter: (params) => `${params.value?.toFixed(1) || 0}%`,
      cellRenderer: (params) => getEfficiencyBadge(params.value)
    },
    {
      headerName: "Created",
      field: "created_at",
      sortable: true,
      filter: true,
      width: 140,
      valueFormatter: (params) => {
        if (!params.value) return '';
        return new Date(params.value).toLocaleDateString();
      }
    },
    {
      headerName: "Select",
      field: "select",
      width: 80,
      cellRenderer: (params) => (
        <input
          type="checkbox"
          checked={selectedPlans.some(p => p.id === params.data.id)}
          onChange={(e) => {
            if (e.target.checked) {
              setSelectedPlans(prev => [...prev, params.data]);
            } else {
              setSelectedPlans(prev => prev.filter(p => p.id !== params.data.id));
            }
          }}
        />
      )
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
          onClick={() => handleGetOptimization(params.data)}
          disabled={analyzing}
        >
          {analyzing && selectedPlanForOptimization?.id === params.data.id ?
            <CSpinner size="sm" /> : "Optimize"}
        </CButton>
      )
    }
  ];

  return (
    <div>
      <PageHeader
        title="Explain Plans"
        breadcrumbs={[
          { label: "Performance Analysis", link: "/performance-analysis" },
          { label: "Explain Plans" }
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
          <h5 className="mb-0">Plan Management</h5>
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
                onClick={() => {
                  resetPlanForm();
                  setShowCreateModal(true);
                }}
                disabled={!selectedCluster}
              >
                Create Plan
              </CButton>
              <CButton
                color="secondary"
                className="me-2"
                onClick={handleComparePlans}
                disabled={selectedPlans.length < 2 || analyzing}
              >
                {analyzing ? <CSpinner size="sm" /> : "Compare Selected"}
              </CButton>
              <CButton
                color="info"
                variant="outline"
                onClick={() => setSelectedPlans([])}
                disabled={selectedPlans.length === 0}
              >
                Clear Selection ({selectedPlans.length})
              </CButton>
            </CCol>
          </CRow>
        </CCardBody>
      </CCard>

      {/* Plans Table */}
      <CCard>
        <CCardHeader>
          <div className="d-flex justify-content-between align-items-center">
            <h5 className="mb-0">Explain Plans</h5>
            <div>
              <span className="text-muted me-2">
                Showing {plans.length} plans
                {selectedCluster && ` for ${selectedCluster.cluster_identifier}`}
              </span>
            </div>
          </div>
        </CCardHeader>
        <CCardBody>
          {loading ? (
            <div className="text-center">
              <CSpinner />
              <div className="mt-2">Loading plans...</div>
            </div>
          ) : (
            <div className="ag-theme-alpine" style={{ height: '600px', width: '100%' }}>
              <AgGridReact
                ref={plansGridRef}
                rowData={plans}
                columnDefs={planColumnDefs}
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

      {/* Create Plan Modal */}
      <CModal size="lg" visible={showCreateModal} onClose={() => setShowCreateModal(false)}>
        <CModalHeader>
          <CModalTitle>Create Explain Plan</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <CForm onSubmit={handleCreatePlan}>
            <CRow className="mb-3">
              <CCol xs={12}>
                <CFormLabel>SQL Query</CFormLabel>
                <CFormTextarea
                  rows={6}
                  value={planForm.query_text}
                  onChange={(e) => setPlanForm({...planForm, query_text: e.target.value})}
                  placeholder="Enter the SQL query to analyze..."
                  required
                />
              </CCol>
            </CRow>
            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel>Database Name</CFormLabel>
                <CFormInput
                  type="text"
                  value={planForm.database_name}
                  onChange={(e) => setPlanForm({...planForm, database_name: e.target.value})}
                  placeholder="Database name (optional)"
                />
              </CCol>
            </CRow>
            <CRow className="mb-3">
              <CCol xs={12}>
                <CFormLabel>Query Parameters (JSON)</CFormLabel>
                <CFormTextarea
                  rows={3}
                  value={planForm.parameters}
                  onChange={(e) => setPlanForm({...planForm, parameters: e.target.value})}
                  placeholder='{"param1": "value1", "param2": "value2"}'
                />
              </CCol>
            </CRow>
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowCreateModal(false)}>
            Cancel
          </CButton>
          <CButton
            color="primary"
            onClick={handleCreatePlan}
            disabled={loading}
          >
            {loading ? <CSpinner size="sm" /> : "Create Plan"}
          </CButton>
        </CModalFooter>
      </CModal>

      {/* Comparison Modal */}
      <CModal size="xl" visible={showComparisonModal} onClose={() => setShowComparisonModal(false)}>
        <CModalHeader>
          <CModalTitle>Plan Comparison</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {comparisonResult && (
            <div>
              <CRow className="mb-4">
                <CCol md={12}>
                  <h5>Comparison Summary</h5>
                  <div style={{
                    backgroundColor: '#f8f9fa',
                    padding: '15px',
                    borderRadius: '4px'
                  }}>
                    <p>{comparisonResult.summary}</p>
                  </div>
                </CCol>
              </CRow>

              {comparisonResult.differences && (
                <CRow className="mb-4">
                  <CCol md={12}>
                    <h5>Key Differences</h5>
                    <ul>
                      {comparisonResult.differences.map((diff, index) => (
                        <li key={index}>{diff}</li>
                      ))}
                    </ul>
                  </CCol>
                </CRow>
              )}

              {comparisonResult.recommendations && (
                <CRow className="mb-4">
                  <CCol md={12}>
                    <h5>Recommendations</h5>
                    <ul>
                      {comparisonResult.recommendations.map((rec, index) => (
                        <li key={index}>{rec}</li>
                      ))}
                    </ul>
                  </CCol>
                </CRow>
              )}
            </div>
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowComparisonModal(false)}>
            Close
          </CButton>
        </CModalFooter>
      </CModal>

      {/* Optimization Modal */}
      <CModal size="xl" visible={showOptimizationModal} onClose={() => setShowOptimizationModal(false)}>
        <CModalHeader>
          <CModalTitle>Plan Optimization Suggestions</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {optimizationResult && (
            <div>
              <CRow className="mb-3">
                <CCol md={12}>
                  <h5>Current Plan Analysis</h5>
                  <div style={{
                    backgroundColor: '#fff3cd',
                    padding: '15px',
                    borderRadius: '4px',
                    borderLeft: '4px solid #ffc107'
                  }}>
                    <p><strong>Efficiency Score:</strong> {optimizationResult.current_efficiency?.toFixed(1)}%</p>
                    <p><strong>Total Cost:</strong> {formatCost(optimizationResult.current_cost)}</p>
                    <p><strong>Execution Time:</strong> {optimizationResult.execution_time_ms?.toFixed(2)}ms</p>
                  </div>
                </CCol>
              </CRow>

              {optimizationResult.suggestions && optimizationResult.suggestions.length > 0 && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>Optimization Suggestions</h5>
                    {optimizationResult.suggestions.map((suggestion, index) => (
                      <div key={index} style={{
                        backgroundColor: '#d1ecf1',
                        padding: '12px',
                        borderRadius: '4px',
                        marginBottom: '8px',
                        borderLeft: '4px solid #17a2b8'
                      }}>
                        <div className="d-flex justify-content-between align-items-start">
                          <div>
                            <h6>{suggestion.title}</h6>
                            <p className="mb-1">{suggestion.description}</p>
                            {suggestion.impact && (
                              <small className="text-muted">
                                <strong>Expected Impact:</strong> {suggestion.impact}
                              </small>
                            )}
                          </div>
                          <CBadge color={suggestion.priority === 'high' ? 'danger' :
                                        suggestion.priority === 'medium' ? 'warning' : 'info'}>
                            {suggestion.priority}
                          </CBadge>
                        </div>
                        {suggestion.sql_example && (
                          <div className="mt-2">
                            <small><strong>Example:</strong></small>
                            <pre style={{
                              backgroundColor: '#f8f9fa',
                              padding: '8px',
                              borderRadius: '4px',
                              fontSize: '11px',
                              marginTop: '4px'
                            }}>
                              {suggestion.sql_example}
                            </pre>
                          </div>
                        )}
                      </div>
                    ))}
                  </CCol>
                </CRow>
              )}

              {optimizationResult.optimized_plan && (
                <CRow className="mb-3">
                  <CCol md={12}>
                    <h5>Optimized Plan Preview</h5>
                    <div style={{
                      backgroundColor: '#d4edda',
                      padding: '15px',
                      borderRadius: '4px',
                      borderLeft: '4px solid #28a745'
                    }}>
                      <p><strong>Estimated Cost:</strong> {formatCost(optimizationResult.optimized_plan.estimated_cost)}</p>
                      <p><strong>Estimated Improvement:</strong> {optimizationResult.optimized_plan.improvement_percentage?.toFixed(1)}%</p>
                    </div>
                  </CCol>
                </CRow>
              )}
            </div>
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowOptimizationModal(false)}>
            Close
          </CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default ExplainPlans;