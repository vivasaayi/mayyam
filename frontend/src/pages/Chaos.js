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

import React, { useState, useEffect, useCallback } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CBadge,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane,
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
  CForm,
  CFormInput,
  CFormSelect,
  CFormTextarea,
  CFormLabel,
  CSpinner,
  CAlert,
  CAccordion,
  CAccordionItem,
  CAccordionHeader,
  CAccordionBody,
  CFormCheck,
  CProgress,
  CProgressBar,
  CCallout,
} from "@coreui/react";
import {
  listTemplates,
  listExperimentsWithRuns,
  listExperiments,
  createExperiment,
  createExperimentFromTemplate,
  runExperiment,
  stopExperiment,
  deleteExperiment,
  batchRunExperiments,
  listExperimentRuns,
  getRun,
  getExperimentResults,
  getExperimentsForResource,
  getResourceExperimentHistory,
} from "../services/chaosService";

const SEVERITY_COLORS = {
  low: "success",
  medium: "warning",
  high: "danger",
  critical: "dark",
  unknown: "secondary",
  none: "info",
};

const STATUS_COLORS = {
  draft: "secondary",
  ready: "info",
  scheduled: "primary",
  running: "warning",
  completed: "success",
  failed: "danger",
  cancelled: "dark",
  pending: "secondary",
  initializing: "info",
  rolling_back: "warning",
  timed_out: "danger",
};

const CATEGORY_ICONS = {
  compute: "🖥️",
  database: "🗄️",
  networking: "🌐",
  storage: "📦",
  serverless: "⚡",
};

const Chaos = () => {
  const [activeTab, setActiveTab] = useState("experiments");
  const [templates, setTemplates] = useState([]);
  const [experiments, setExperiments] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Modal states
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showTemplateModal, setShowTemplateModal] = useState(false);
  const [showRunModal, setShowRunModal] = useState(false);
  const [showResultsModal, setShowResultsModal] = useState(false);
  const [showResourceModal, setShowResourceModal] = useState(false);

  // Selected items
  const [selectedTemplate, setSelectedTemplate] = useState(null);
  const [selectedExperiment, setSelectedExperiment] = useState(null);
  const [selectedRuns, setSelectedRuns] = useState([]);
  const [selectedResults, setSelectedResults] = useState([]);
  const [selectedRunDetail, setSelectedRunDetail] = useState(null);
  const [resourceHistory, setResourceHistory] = useState(null);

  // Filters
  const [categoryFilter, setCategoryFilter] = useState("");
  const [resourceTypeFilter, setResourceTypeFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");

  // Create form
  const [createForm, setCreateForm] = useState({
    name: "",
    description: "",
    account_id: "",
    region: "",
    resource_type: "",
    target_resource_id: "",
    target_resource_name: "",
    experiment_type: "",
    dry_run: true,
    duration_seconds: 60,
  });

  // Batch selection
  const [selectedExperimentIds, setSelectedExperimentIds] = useState([]);

  // Resource search
  const [resourceSearchId, setResourceSearchId] = useState("");

  const loadTemplates = useCallback(async () => {
    try {
      setLoading(true);
      const params = {};
      if (categoryFilter) params.category = categoryFilter;
      if (resourceTypeFilter) params.resource_type = resourceTypeFilter;
      const res = await listTemplates(params);
      setTemplates(res.data || []);
    } catch (err) {
      setError("Failed to load templates: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  }, [categoryFilter, resourceTypeFilter]);

  const loadExperiments = useCallback(async () => {
    try {
      setLoading(true);
      const params = {};
      if (statusFilter) params.status = statusFilter;
      if (resourceTypeFilter) params.resource_type = resourceTypeFilter;
      const res = await listExperimentsWithRuns(params);
      setExperiments(res.data || []);
    } catch (err) {
      setError("Failed to load experiments: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  }, [statusFilter, resourceTypeFilter]);

  useEffect(() => {
    if (activeTab === "templates") {
      loadTemplates();
    } else if (activeTab === "experiments") {
      loadExperiments();
    }
  }, [activeTab, loadTemplates, loadExperiments]);

  const clearMessages = () => {
    setError(null);
    setSuccess(null);
  };

  // Template actions
  const handleUseTemplate = (template) => {
    setSelectedTemplate(template);
    setCreateForm({
      ...createForm,
      name: template.name,
      description: template.description || "",
      resource_type: template.resource_type,
      experiment_type: template.experiment_type,
      duration_seconds: template.estimated_duration_seconds || 60,
    });
    setShowTemplateModal(true);
  };

  const handleCreateFromTemplate = async () => {
    try {
      setLoading(true);
      clearMessages();
      await createExperimentFromTemplate(selectedTemplate.id, {
        account_id: createForm.account_id,
        region: createForm.region,
        target_resource_id: createForm.target_resource_id,
        target_resource_name: createForm.target_resource_name || null,
        parameter_overrides: {
          dry_run: createForm.dry_run,
          duration_seconds: parseInt(createForm.duration_seconds) || 60,
        },
      });
      setSuccess("Experiment created from template successfully!");
      setShowTemplateModal(false);
      setActiveTab("experiments");
      loadExperiments();
    } catch (err) {
      setError("Failed to create experiment: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  // Experiment actions
  const handleCreateExperiment = async () => {
    try {
      setLoading(true);
      clearMessages();
      await createExperiment({
        name: createForm.name,
        description: createForm.description || null,
        account_id: createForm.account_id,
        region: createForm.region,
        resource_type: createForm.resource_type,
        target_resource_id: createForm.target_resource_id,
        target_resource_name: createForm.target_resource_name || null,
        experiment_type: createForm.experiment_type,
        parameters: {
          dry_run: createForm.dry_run,
          duration_seconds: parseInt(createForm.duration_seconds) || 60,
          rollback_on_failure: true,
        },
      });
      setSuccess("Experiment created successfully!");
      setShowCreateModal(false);
      loadExperiments();
    } catch (err) {
      setError("Failed to create experiment: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleRunExperiment = async (experimentId) => {
    try {
      setLoading(true);
      clearMessages();
      const res = await runExperiment(experimentId, {
        triggered_by: "ui_user",
      });
      setSuccess(`Experiment run started! Run ID: ${res.data?.id}`);
      loadExperiments();
    } catch (err) {
      setError("Failed to run experiment: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleStopExperiment = async (experimentId) => {
    try {
      setLoading(true);
      clearMessages();
      await stopExperiment(experimentId);
      setSuccess("Experiment stopped successfully!");
      loadExperiments();
    } catch (err) {
      setError("Failed to stop experiment: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteExperiment = async (experimentId) => {
    if (!window.confirm("Are you sure you want to delete this experiment?")) return;
    try {
      setLoading(true);
      clearMessages();
      await deleteExperiment(experimentId);
      setSuccess("Experiment deleted!");
      loadExperiments();
    } catch (err) {
      setError("Failed to delete experiment: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleBatchRun = async () => {
    if (selectedExperimentIds.length === 0) {
      setError("Select at least one experiment to run");
      return;
    }
    try {
      setLoading(true);
      clearMessages();
      const res = await batchRunExperiments({
        experiment_ids: selectedExperimentIds,
        triggered_by: "ui_user_batch",
      });
      setSuccess(`Batch run started! ${res.data?.length || 0} experiments initiated.`);
      setSelectedExperimentIds([]);
      loadExperiments();
    } catch (err) {
      setError("Failed to batch run: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  // View run details
  const handleViewRuns = async (experiment) => {
    try {
      setLoading(true);
      clearMessages();
      setSelectedExperiment(experiment);
      const res = await listExperimentRuns(experiment.id || experiment.experiment?.id);
      setSelectedRuns(res.data || []);
      setShowRunModal(true);
    } catch (err) {
      setError("Failed to load runs: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleViewRunDetail = async (runId) => {
    try {
      setLoading(true);
      const res = await getRun(runId);
      setSelectedRunDetail(res.data);
    } catch (err) {
      setError("Failed to load run details: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  // View results
  const handleViewResults = async (experiment) => {
    try {
      setLoading(true);
      clearMessages();
      setSelectedExperiment(experiment);
      const expId = experiment.id || experiment.experiment?.id;
      const res = await getExperimentResults(expId);
      setSelectedResults(res.data || []);
      setShowResultsModal(true);
    } catch (err) {
      setError("Failed to load results: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  // Resource history
  const handleSearchResource = async () => {
    if (!resourceSearchId.trim()) {
      setError("Enter a resource ID to search");
      return;
    }
    try {
      setLoading(true);
      clearMessages();
      const res = await getResourceExperimentHistory(resourceSearchId.trim());
      setResourceHistory(res.data);
      setShowResourceModal(true);
    } catch (err) {
      setError("Failed to load resource history: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const toggleExperimentSelection = (id) => {
    setSelectedExperimentIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]
    );
  };

  // Group templates by category
  const templatesByCategory = templates.reduce((acc, t) => {
    const cat = t.category || "other";
    if (!acc[cat]) acc[cat] = [];
    acc[cat].push(t);
    return acc;
  }, {});

  return (
    <>
      <h2 className="mb-4">Chaos Engineering</h2>

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

      {/* Navigation Tabs */}
      <CNav variant="tabs" className="mb-4">
        <CNavItem>
          <CNavLink
            active={activeTab === "experiments"}
            onClick={() => setActiveTab("experiments")}
            style={{ cursor: "pointer" }}
          >
            Experiments
          </CNavLink>
        </CNavItem>
        <CNavItem>
          <CNavLink
            active={activeTab === "templates"}
            onClick={() => setActiveTab("templates")}
            style={{ cursor: "pointer" }}
          >
            Template Gallery
          </CNavLink>
        </CNavItem>
        <CNavItem>
          <CNavLink
            active={activeTab === "resources"}
            onClick={() => setActiveTab("resources")}
            style={{ cursor: "pointer" }}
          >
            Resource Browser
          </CNavLink>
        </CNavItem>
      </CNav>

      <CTabContent>
        {/* ================================================================ */}
        {/* EXPERIMENTS TAB */}
        {/* ================================================================ */}
        <CTabPane visible={activeTab === "experiments"}>
          {/* Filters & Actions Bar */}
          <CCard className="mb-3">
            <CCardBody>
              <CRow className="align-items-end">
                <CCol md={3}>
                  <CFormLabel>Status</CFormLabel>
                  <CFormSelect
                    value={statusFilter}
                    onChange={(e) => setStatusFilter(e.target.value)}
                  >
                    <option value="">All Statuses</option>
                    <option value="draft">Draft</option>
                    <option value="ready">Ready</option>
                    <option value="running">Running</option>
                    <option value="completed">Completed</option>
                    <option value="failed">Failed</option>
                    <option value="cancelled">Cancelled</option>
                  </CFormSelect>
                </CCol>
                <CCol md={3}>
                  <CFormLabel>Resource Type</CFormLabel>
                  <CFormSelect
                    value={resourceTypeFilter}
                    onChange={(e) => setResourceTypeFilter(e.target.value)}
                  >
                    <option value="">All Types</option>
                    <option value="EC2Instance">EC2 Instance</option>
                    <option value="RdsInstance">RDS Instance</option>
                    <option value="LambdaFunction">Lambda Function</option>
                    <option value="EcsService">ECS Service</option>
                    <option value="ElasticacheCluster">ElastiCache</option>
                    <option value="DynamoDbTable">DynamoDB Table</option>
                    <option value="S3Bucket">S3 Bucket</option>
                    <option value="Alb">ALB</option>
                    <option value="SecurityGroup">Security Group</option>
                    <option value="SqsQueue">SQS Queue</option>
                    <option value="EksCluster">EKS Cluster</option>
                  </CFormSelect>
                </CCol>
                <CCol md={6} className="d-flex gap-2 justify-content-end">
                  <CButton
                    color="primary"
                    onClick={() => {
                      setCreateForm({
                        name: "",
                        description: "",
                        account_id: "",
                        region: "",
                        resource_type: "",
                        target_resource_id: "",
                        target_resource_name: "",
                        experiment_type: "",
                        dry_run: true,
                        duration_seconds: 60,
                      });
                      setShowCreateModal(true);
                    }}
                  >
                    + New Experiment
                  </CButton>
                  {selectedExperimentIds.length > 0 && (
                    <CButton color="warning" onClick={handleBatchRun}>
                      Run Selected ({selectedExperimentIds.length})
                    </CButton>
                  )}
                  <CButton
                    color="light"
                    onClick={() => {
                      loadExperiments();
                      clearMessages();
                    }}
                  >
                    Refresh
                  </CButton>
                </CCol>
              </CRow>
            </CCardBody>
          </CCard>

          {/* Experiments Table */}
          {loading ? (
            <div className="text-center py-5">
              <CSpinner color="primary" />
            </div>
          ) : experiments.length === 0 ? (
            <CCallout color="info">
              No experiments configured yet. Create one using the "+ New Experiment" button or
              browse the Template Gallery to get started quickly.
            </CCallout>
          ) : (
            <CCard>
              <CCardBody>
                <CTable hover responsive>
                  <CTableHead>
                    <CTableRow>
                      <CTableHeaderCell style={{ width: "40px" }}>
                        <CFormCheck
                          checked={
                            selectedExperimentIds.length === experiments.length &&
                            experiments.length > 0
                          }
                          onChange={() => {
                            if (selectedExperimentIds.length === experiments.length) {
                              setSelectedExperimentIds([]);
                            } else {
                              setSelectedExperimentIds(
                                experiments.map((e) => e.id || e.experiment?.id)
                              );
                            }
                          }}
                        />
                      </CTableHeaderCell>
                      <CTableHeaderCell>Name</CTableHeaderCell>
                      <CTableHeaderCell>Type</CTableHeaderCell>
                      <CTableHeaderCell>Target Resource</CTableHeaderCell>
                      <CTableHeaderCell>Region</CTableHeaderCell>
                      <CTableHeaderCell>Status</CTableHeaderCell>
                      <CTableHeaderCell>Last Run</CTableHeaderCell>
                      <CTableHeaderCell>Total Runs</CTableHeaderCell>
                      <CTableHeaderCell>Actions</CTableHeaderCell>
                    </CTableRow>
                  </CTableHead>
                  <CTableBody>
                    {experiments.map((item) => {
                      const exp = item.experiment || item;
                      const lastRun = item.last_run;
                      const totalRuns = item.total_runs || 0;
                      return (
                        <CTableRow key={exp.id}>
                          <CTableDataCell>
                            <CFormCheck
                              checked={selectedExperimentIds.includes(exp.id)}
                              onChange={() => toggleExperimentSelection(exp.id)}
                            />
                          </CTableDataCell>
                          <CTableDataCell>
                            <strong>{exp.name}</strong>
                            {exp.description && (
                              <div className="small text-muted">{exp.description}</div>
                            )}
                          </CTableDataCell>
                          <CTableDataCell>
                            <code>{exp.experiment_type}</code>
                          </CTableDataCell>
                          <CTableDataCell>
                            <code className="small">{exp.target_resource_id}</code>
                            {exp.target_resource_name && (
                              <div className="small text-muted">{exp.target_resource_name}</div>
                            )}
                          </CTableDataCell>
                          <CTableDataCell>{exp.region}</CTableDataCell>
                          <CTableDataCell>
                            <CBadge color={STATUS_COLORS[exp.status] || "secondary"}>
                              {exp.status}
                            </CBadge>
                          </CTableDataCell>
                          <CTableDataCell>
                            {lastRun ? (
                              <>
                                <CBadge
                                  color={STATUS_COLORS[lastRun.status] || "secondary"}
                                  className="me-1"
                                >
                                  {lastRun.status}
                                </CBadge>
                                <div className="small text-muted">
                                  {lastRun.started_at
                                    ? new Date(lastRun.started_at).toLocaleDateString()
                                    : "—"}
                                </div>
                              </>
                            ) : (
                              <span className="text-muted">Never</span>
                            )}
                          </CTableDataCell>
                          <CTableDataCell>{totalRuns}</CTableDataCell>
                          <CTableDataCell>
                            <div className="d-flex gap-1 flex-wrap">
                              {exp.status !== "running" ? (
                                <CButton
                                  color="success"
                                  size="sm"
                                  onClick={() => handleRunExperiment(exp.id)}
                                >
                                  Run
                                </CButton>
                              ) : (
                                <CButton
                                  color="danger"
                                  size="sm"
                                  onClick={() => handleStopExperiment(exp.id)}
                                >
                                  Stop
                                </CButton>
                              )}
                              <CButton
                                color="info"
                                size="sm"
                                variant="outline"
                                onClick={() => handleViewRuns(item)}
                              >
                                Runs
                              </CButton>
                              <CButton
                                color="primary"
                                size="sm"
                                variant="outline"
                                onClick={() => handleViewResults(item)}
                              >
                                Results
                              </CButton>
                              <CButton
                                color="danger"
                                size="sm"
                                variant="outline"
                                onClick={() => handleDeleteExperiment(exp.id)}
                              >
                                Delete
                              </CButton>
                            </div>
                          </CTableDataCell>
                        </CTableRow>
                      );
                    })}
                  </CTableBody>
                </CTable>
              </CCardBody>
            </CCard>
          )}
        </CTabPane>

        {/* ================================================================ */}
        {/* TEMPLATE GALLERY TAB */}
        {/* ================================================================ */}
        <CTabPane visible={activeTab === "templates"}>
          <CCard className="mb-3">
            <CCardBody>
              <CRow className="align-items-end">
                <CCol md={4}>
                  <CFormLabel>Category</CFormLabel>
                  <CFormSelect
                    value={categoryFilter}
                    onChange={(e) => setCategoryFilter(e.target.value)}
                  >
                    <option value="">All Categories</option>
                    <option value="compute">Compute</option>
                    <option value="database">Database</option>
                    <option value="networking">Networking</option>
                    <option value="storage">Storage</option>
                    <option value="serverless">Serverless</option>
                  </CFormSelect>
                </CCol>
                <CCol md={4}>
                  <CFormLabel>Resource Type</CFormLabel>
                  <CFormSelect
                    value={resourceTypeFilter}
                    onChange={(e) => setResourceTypeFilter(e.target.value)}
                  >
                    <option value="">All Types</option>
                    <option value="EC2Instance">EC2 Instance</option>
                    <option value="RdsInstance">RDS Instance</option>
                    <option value="LambdaFunction">Lambda Function</option>
                    <option value="EcsService">ECS Service</option>
                    <option value="ElasticacheCluster">ElastiCache</option>
                    <option value="DynamoDbTable">DynamoDB Table</option>
                    <option value="S3Bucket">S3 Bucket</option>
                    <option value="Alb">ALB</option>
                    <option value="SecurityGroup">Security Group</option>
                    <option value="SqsQueue">SQS Queue</option>
                    <option value="EksCluster">EKS Cluster</option>
                  </CFormSelect>
                </CCol>
                <CCol md={4} className="d-flex justify-content-end">
                  <CButton color="light" onClick={loadTemplates}>
                    Refresh
                  </CButton>
                </CCol>
              </CRow>
            </CCardBody>
          </CCard>

          {loading ? (
            <div className="text-center py-5">
              <CSpinner color="primary" />
            </div>
          ) : (
            <CAccordion alwaysOpen>
              {Object.entries(templatesByCategory).map(([category, catTemplates]) => (
                <CAccordionItem key={category}>
                  <CAccordionHeader>
                    <span className="me-2">{CATEGORY_ICONS[category] || "📋"}</span>
                    <strong className="text-capitalize">{category}</strong>
                    <CBadge color="primary" className="ms-2">
                      {catTemplates.length}
                    </CBadge>
                  </CAccordionHeader>
                  <CAccordionBody>
                    <CRow>
                      {catTemplates.map((template) => (
                        <CCol md={6} lg={4} key={template.id} className="mb-3">
                          <CCard className="h-100">
                            <CCardHeader className="d-flex justify-content-between align-items-center">
                              <strong>{template.name}</strong>
                              <CBadge
                                color={
                                  SEVERITY_COLORS[template.expected_impact] || "secondary"
                                }
                              >
                                {template.expected_impact}
                              </CBadge>
                            </CCardHeader>
                            <CCardBody>
                              <p className="small text-muted mb-2">{template.description}</p>
                              <div className="mb-2">
                                <small className="text-muted">Resource: </small>
                                <code>{template.resource_type}</code>
                              </div>
                              <div className="mb-2">
                                <small className="text-muted">Type: </small>
                                <code>{template.experiment_type}</code>
                              </div>
                              <div className="mb-2">
                                <small className="text-muted">Duration: </small>
                                {template.estimated_duration_seconds}s
                              </div>
                              {template.prerequisites && template.prerequisites.length > 0 && (
                                <div className="mb-2">
                                  <small className="text-muted d-block">Prerequisites:</small>
                                  <ul className="small mb-0 ps-3">
                                    {template.prerequisites.map((p, i) => (
                                      <li key={i}>{p}</li>
                                    ))}
                                  </ul>
                                </div>
                              )}
                              <CButton
                                color="primary"
                                size="sm"
                                className="mt-2 w-100"
                                onClick={() => handleUseTemplate(template)}
                              >
                                Use Template
                              </CButton>
                            </CCardBody>
                          </CCard>
                        </CCol>
                      ))}
                    </CRow>
                  </CAccordionBody>
                </CAccordionItem>
              ))}
            </CAccordion>
          )}
        </CTabPane>

        {/* ================================================================ */}
        {/* RESOURCE BROWSER TAB */}
        {/* ================================================================ */}
        <CTabPane visible={activeTab === "resources"}>
          <CCard>
            <CCardHeader>Search Resource Experiment History</CCardHeader>
            <CCardBody>
              <CRow className="align-items-end mb-4">
                <CCol md={8}>
                  <CFormLabel>AWS Resource ID</CFormLabel>
                  <CFormInput
                    placeholder="Enter resource ID (e.g., i-0abc123, my-rds-cluster, my-lambda-function)"
                    value={resourceSearchId}
                    onChange={(e) => setResourceSearchId(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleSearchResource()}
                  />
                </CCol>
                <CCol md={4}>
                  <CButton
                    color="primary"
                    onClick={handleSearchResource}
                    disabled={loading}
                  >
                    {loading ? <CSpinner size="sm" className="me-1" /> : null}
                    Search History
                  </CButton>
                </CCol>
              </CRow>

              {resourceHistory && (
                <div>
                  <h5>
                    Resource: <code>{resourceHistory.resource_id}</code>
                  </h5>
                  {resourceHistory.resource_type && (
                    <p className="text-muted">
                      Type: {resourceHistory.resource_type} | Total Runs:{" "}
                      {resourceHistory.total_runs}
                      {resourceHistory.last_run_at && (
                        <>
                          {" "}
                          | Last Run:{" "}
                          {new Date(resourceHistory.last_run_at).toLocaleString()}
                        </>
                      )}
                    </p>
                  )}

                  {resourceHistory.experiments && resourceHistory.experiments.length > 0 ? (
                    <CTable hover responsive>
                      <CTableHead>
                        <CTableRow>
                          <CTableHeaderCell>Experiment</CTableHeaderCell>
                          <CTableHeaderCell>Type</CTableHeaderCell>
                          <CTableHeaderCell>Status</CTableHeaderCell>
                          <CTableHeaderCell>Started</CTableHeaderCell>
                          <CTableHeaderCell>Ended</CTableHeaderCell>
                          <CTableHeaderCell>Impact</CTableHeaderCell>
                          <CTableHeaderCell>Recovery Time</CTableHeaderCell>
                        </CTableRow>
                      </CTableHead>
                      <CTableBody>
                        {resourceHistory.experiments.map((exp, i) => (
                          <CTableRow key={i}>
                            <CTableDataCell>{exp.experiment_name}</CTableDataCell>
                            <CTableDataCell>
                              <code>{exp.experiment_type}</code>
                            </CTableDataCell>
                            <CTableDataCell>
                              <CBadge
                                color={STATUS_COLORS[exp.run_status] || "secondary"}
                              >
                                {exp.run_status}
                              </CBadge>
                            </CTableDataCell>
                            <CTableDataCell>
                              {exp.started_at
                                ? new Date(exp.started_at).toLocaleString()
                                : "—"}
                            </CTableDataCell>
                            <CTableDataCell>
                              {exp.ended_at
                                ? new Date(exp.ended_at).toLocaleString()
                                : "—"}
                            </CTableDataCell>
                            <CTableDataCell>
                              <CBadge
                                color={SEVERITY_COLORS[exp.impact_severity] || "secondary"}
                              >
                                {exp.impact_severity}
                              </CBadge>
                            </CTableDataCell>
                            <CTableDataCell>
                              {exp.recovery_time_ms
                                ? `${(exp.recovery_time_ms / 1000).toFixed(1)}s`
                                : "—"}
                            </CTableDataCell>
                          </CTableRow>
                        ))}
                      </CTableBody>
                    </CTable>
                  ) : (
                    <CCallout color="info">
                      No chaos experiments have been conducted on this resource.
                    </CCallout>
                  )}
                </div>
              )}
            </CCardBody>
          </CCard>
        </CTabPane>
      </CTabContent>

      {/* ================================================================ */}
      {/* CREATE EXPERIMENT MODAL */}
      {/* ================================================================ */}
      <CModal
        visible={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        size="lg"
      >
        <CModalHeader>
          <CModalTitle>Create Chaos Experiment</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <CRow className="mb-3">
            <CCol md={6}>
              <CFormLabel>Name *</CFormLabel>
              <CFormInput
                value={createForm.name}
                onChange={(e) => setCreateForm({ ...createForm, name: e.target.value })}
                placeholder="e.g., Production RDS Failover Test"
              />
            </CCol>
            <CCol md={6}>
              <CFormLabel>Experiment Type *</CFormLabel>
              <CFormSelect
                value={createForm.experiment_type}
                onChange={(e) =>
                  setCreateForm({ ...createForm, experiment_type: e.target.value })
                }
              >
                <option value="">Select type...</option>
                <option value="instance_stop">EC2 Instance Stop</option>
                <option value="instance_reboot">EC2 Instance Reboot</option>
                <option value="instance_terminate">EC2 Instance Terminate</option>
                <option value="rds_failover">RDS Failover</option>
                <option value="rds_reboot">RDS Reboot</option>
                <option value="lambda_disable">Lambda Disable</option>
                <option value="lambda_timeout">Lambda Timeout Reduction</option>
                <option value="ecs_scale_down">ECS Scale Down</option>
                <option value="elasticache_failover">ElastiCache Failover</option>
                <option value="dynamodb_throttle">DynamoDB Throttle</option>
                <option value="s3_deny_access">S3 Deny Access</option>
                <option value="alb_deregister_targets">ALB Deregister Targets</option>
                <option value="sg_block_ingress">SG Block Ingress</option>
                <option value="sqs_purge">SQS Purge</option>
                <option value="eks_scale_down">EKS Scale Down</option>
              </CFormSelect>
            </CCol>
          </CRow>
          <CRow className="mb-3">
            <CCol md={12}>
              <CFormLabel>Description</CFormLabel>
              <CFormTextarea
                value={createForm.description}
                onChange={(e) =>
                  setCreateForm({ ...createForm, description: e.target.value })
                }
                rows={2}
                placeholder="Describe the purpose of this experiment..."
              />
            </CCol>
          </CRow>
          <CRow className="mb-3">
            <CCol md={6}>
              <CFormLabel>AWS Account ID *</CFormLabel>
              <CFormInput
                value={createForm.account_id}
                onChange={(e) =>
                  setCreateForm({ ...createForm, account_id: e.target.value })
                }
                placeholder="e.g., 123456789012"
              />
            </CCol>
            <CCol md={6}>
              <CFormLabel>Region *</CFormLabel>
              <CFormSelect
                value={createForm.region}
                onChange={(e) => setCreateForm({ ...createForm, region: e.target.value })}
              >
                <option value="">Select region...</option>
                <option value="us-east-1">us-east-1</option>
                <option value="us-east-2">us-east-2</option>
                <option value="us-west-1">us-west-1</option>
                <option value="us-west-2">us-west-2</option>
                <option value="eu-west-1">eu-west-1</option>
                <option value="eu-west-2">eu-west-2</option>
                <option value="eu-central-1">eu-central-1</option>
                <option value="ap-southeast-1">ap-southeast-1</option>
                <option value="ap-southeast-2">ap-southeast-2</option>
                <option value="ap-northeast-1">ap-northeast-1</option>
              </CFormSelect>
            </CCol>
          </CRow>
          <CRow className="mb-3">
            <CCol md={6}>
              <CFormLabel>Resource Type *</CFormLabel>
              <CFormSelect
                value={createForm.resource_type}
                onChange={(e) =>
                  setCreateForm({ ...createForm, resource_type: e.target.value })
                }
              >
                <option value="">Select resource type...</option>
                <option value="EC2Instance">EC2 Instance</option>
                <option value="RdsInstance">RDS Instance</option>
                <option value="LambdaFunction">Lambda Function</option>
                <option value="EcsService">ECS Service</option>
                <option value="ElasticacheCluster">ElastiCache</option>
                <option value="DynamoDbTable">DynamoDB Table</option>
                <option value="S3Bucket">S3 Bucket</option>
                <option value="Alb">ALB</option>
                <option value="SecurityGroup">Security Group</option>
                <option value="SqsQueue">SQS Queue</option>
                <option value="EksCluster">EKS Cluster</option>
              </CFormSelect>
            </CCol>
            <CCol md={6}>
              <CFormLabel>Target Resource ID *</CFormLabel>
              <CFormInput
                value={createForm.target_resource_id}
                onChange={(e) =>
                  setCreateForm({ ...createForm, target_resource_id: e.target.value })
                }
                placeholder="e.g., i-0abc123def456, my-rds-cluster"
              />
            </CCol>
          </CRow>
          <CRow className="mb-3">
            <CCol md={4}>
              <CFormLabel>Target Resource Name</CFormLabel>
              <CFormInput
                value={createForm.target_resource_name}
                onChange={(e) =>
                  setCreateForm({ ...createForm, target_resource_name: e.target.value })
                }
                placeholder="Friendly name (optional)"
              />
            </CCol>
            <CCol md={4}>
              <CFormLabel>Duration (seconds)</CFormLabel>
              <CFormInput
                type="number"
                value={createForm.duration_seconds}
                onChange={(e) =>
                  setCreateForm({ ...createForm, duration_seconds: e.target.value })
                }
              />
            </CCol>
            <CCol md={4} className="d-flex align-items-end">
              <CFormCheck
                label="Dry Run Mode"
                checked={createForm.dry_run}
                onChange={(e) =>
                  setCreateForm({ ...createForm, dry_run: e.target.checked })
                }
              />
            </CCol>
          </CRow>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowCreateModal(false)}>
            Cancel
          </CButton>
          <CButton
            color="primary"
            onClick={handleCreateExperiment}
            disabled={
              loading ||
              !createForm.name ||
              !createForm.account_id ||
              !createForm.region ||
              !createForm.resource_type ||
              !createForm.target_resource_id ||
              !createForm.experiment_type
            }
          >
            {loading ? <CSpinner size="sm" className="me-1" /> : null}
            Create Experiment
          </CButton>
        </CModalFooter>
      </CModal>

      {/* ================================================================ */}
      {/* CREATE FROM TEMPLATE MODAL */}
      {/* ================================================================ */}
      <CModal
        visible={showTemplateModal}
        onClose={() => setShowTemplateModal(false)}
        size="lg"
      >
        <CModalHeader>
          <CModalTitle>
            Create Experiment from Template: {selectedTemplate?.name}
          </CModalTitle>
        </CModalHeader>
        <CModalBody>
          {selectedTemplate && (
            <>
              <CCallout color="info" className="mb-3">
                <strong>Template:</strong> {selectedTemplate.description}
                <br />
                <strong>Impact:</strong>{" "}
                <CBadge color={SEVERITY_COLORS[selectedTemplate.expected_impact]}>
                  {selectedTemplate.expected_impact}
                </CBadge>
                <br />
                <strong>Duration:</strong> {selectedTemplate.estimated_duration_seconds}s
              </CCallout>
              <CRow className="mb-3">
                <CCol md={6}>
                  <CFormLabel>AWS Account ID *</CFormLabel>
                  <CFormInput
                    value={createForm.account_id}
                    onChange={(e) =>
                      setCreateForm({ ...createForm, account_id: e.target.value })
                    }
                    placeholder="e.g., 123456789012"
                  />
                </CCol>
                <CCol md={6}>
                  <CFormLabel>Region *</CFormLabel>
                  <CFormSelect
                    value={createForm.region}
                    onChange={(e) =>
                      setCreateForm({ ...createForm, region: e.target.value })
                    }
                  >
                    <option value="">Select region...</option>
                    <option value="us-east-1">us-east-1</option>
                    <option value="us-east-2">us-east-2</option>
                    <option value="us-west-1">us-west-1</option>
                    <option value="us-west-2">us-west-2</option>
                    <option value="eu-west-1">eu-west-1</option>
                    <option value="eu-central-1">eu-central-1</option>
                    <option value="ap-southeast-1">ap-southeast-1</option>
                    <option value="ap-northeast-1">ap-northeast-1</option>
                  </CFormSelect>
                </CCol>
              </CRow>
              <CRow className="mb-3">
                <CCol md={6}>
                  <CFormLabel>Target Resource ID *</CFormLabel>
                  <CFormInput
                    value={createForm.target_resource_id}
                    onChange={(e) =>
                      setCreateForm({
                        ...createForm,
                        target_resource_id: e.target.value,
                      })
                    }
                    placeholder={`Enter ${selectedTemplate.resource_type} ID`}
                  />
                </CCol>
                <CCol md={6}>
                  <CFormLabel>Target Resource Name</CFormLabel>
                  <CFormInput
                    value={createForm.target_resource_name}
                    onChange={(e) =>
                      setCreateForm({
                        ...createForm,
                        target_resource_name: e.target.value,
                      })
                    }
                    placeholder="Friendly name (optional)"
                  />
                </CCol>
              </CRow>
              <CRow className="mb-3">
                <CCol md={4}>
                  <CFormCheck
                    label="Dry Run Mode (recommended for first run)"
                    checked={createForm.dry_run}
                    onChange={(e) =>
                      setCreateForm({ ...createForm, dry_run: e.target.checked })
                    }
                  />
                </CCol>
              </CRow>
            </>
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowTemplateModal(false)}>
            Cancel
          </CButton>
          <CButton
            color="primary"
            onClick={handleCreateFromTemplate}
            disabled={
              loading ||
              !createForm.account_id ||
              !createForm.region ||
              !createForm.target_resource_id
            }
          >
            {loading ? <CSpinner size="sm" className="me-1" /> : null}
            Create Experiment
          </CButton>
        </CModalFooter>
      </CModal>

      {/* ================================================================ */}
      {/* RUNS MODAL */}
      {/* ================================================================ */}
      <CModal
        visible={showRunModal}
        onClose={() => {
          setShowRunModal(false);
          setSelectedRunDetail(null);
        }}
        size="xl"
      >
        <CModalHeader>
          <CModalTitle>
            Experiment Runs:{" "}
            {selectedExperiment?.name || selectedExperiment?.experiment?.name}
          </CModalTitle>
        </CModalHeader>
        <CModalBody>
          {selectedRunDetail ? (
            <div>
              <CButton
                color="link"
                className="mb-3 ps-0"
                onClick={() => setSelectedRunDetail(null)}
              >
                &larr; Back to runs list
              </CButton>
              <h5>
                Run #{selectedRunDetail.run?.run_number || selectedRunDetail.run_number}
              </h5>
              <CRow className="mb-3">
                <CCol md={3}>
                  <strong>Status:</strong>{" "}
                  <CBadge
                    color={
                      STATUS_COLORS[
                        selectedRunDetail.run?.status || selectedRunDetail.status
                      ] || "secondary"
                    }
                  >
                    {selectedRunDetail.run?.status || selectedRunDetail.status}
                  </CBadge>
                </CCol>
                <CCol md={3}>
                  <strong>Duration:</strong>{" "}
                  {selectedRunDetail.run?.duration_ms || selectedRunDetail.duration_ms
                    ? `${((selectedRunDetail.run?.duration_ms || selectedRunDetail.duration_ms) / 1000).toFixed(1)}s`
                    : "—"}
                </CCol>
                <CCol md={3}>
                  <strong>Triggered By:</strong>{" "}
                  {selectedRunDetail.run?.triggered_by ||
                    selectedRunDetail.triggered_by ||
                    "—"}
                </CCol>
                <CCol md={3}>
                  <strong>Rollback:</strong>{" "}
                  {selectedRunDetail.run?.rollback_status ||
                    selectedRunDetail.rollback_status ||
                    "N/A"}
                </CCol>
              </CRow>

              {/* Execution Log */}
              <h6>Execution Log</h6>
              <CCard className="mb-3">
                <CCardBody
                  style={{
                    maxHeight: "300px",
                    overflow: "auto",
                    backgroundColor: "#1e1e1e",
                    color: "#d4d4d4",
                    fontFamily: "monospace",
                    fontSize: "0.85rem",
                  }}
                >
                  {(
                    selectedRunDetail.run?.execution_log ||
                    selectedRunDetail.execution_log ||
                    []
                  ).map((entry, i) => (
                    <div key={i} className="mb-1">
                      <span style={{ color: "#6a9955" }}>
                        [{entry.timestamp ? new Date(entry.timestamp).toLocaleTimeString() : ""}]
                      </span>{" "}
                      <span
                        style={{
                          color:
                            entry.level === "error"
                              ? "#f44747"
                              : entry.level === "warn"
                              ? "#cca700"
                              : "#569cd6",
                        }}
                      >
                        [{entry.level?.toUpperCase()}]
                      </span>{" "}
                      {entry.message}
                    </div>
                  ))}
                </CCardBody>
              </CCard>

              {/* Results */}
              {selectedRunDetail.results && selectedRunDetail.results.length > 0 && (
                <>
                  <h6>Results</h6>
                  {selectedRunDetail.results.map((result, i) => (
                    <CCard key={i} className="mb-2">
                      <CCardBody>
                        <CRow>
                          <CCol md={4}>
                            <strong>Impact:</strong>{" "}
                            <CBadge
                              color={
                                SEVERITY_COLORS[result.impact_severity] || "secondary"
                              }
                            >
                              {result.impact_severity}
                            </CBadge>
                          </CCol>
                          <CCol md={4}>
                            <strong>Recovery Time:</strong>{" "}
                            {result.recovery_time_ms
                              ? `${(result.recovery_time_ms / 1000).toFixed(1)}s`
                              : "—"}
                          </CCol>
                          <CCol md={4}>
                            <strong>Hypothesis Met:</strong>{" "}
                            {result.hypothesis_met === true
                              ? "Yes"
                              : result.hypothesis_met === false
                              ? "No"
                              : "—"}
                          </CCol>
                        </CRow>
                        {result.impact_summary && (
                          <div className="mt-2">
                            <strong>Summary:</strong> {result.impact_summary}
                          </div>
                        )}
                      </CCardBody>
                    </CCard>
                  ))}
                </>
              )}

              {selectedRunDetail.run?.error_message ||
              selectedRunDetail.error_message ? (
                <CAlert color="danger">
                  <strong>Error:</strong>{" "}
                  {selectedRunDetail.run?.error_message ||
                    selectedRunDetail.error_message}
                </CAlert>
              ) : null}
            </div>
          ) : (
            <CTable hover responsive>
              <CTableHead>
                <CTableRow>
                  <CTableHeaderCell>#</CTableHeaderCell>
                  <CTableHeaderCell>Status</CTableHeaderCell>
                  <CTableHeaderCell>Started</CTableHeaderCell>
                  <CTableHeaderCell>Duration</CTableHeaderCell>
                  <CTableHeaderCell>Triggered By</CTableHeaderCell>
                  <CTableHeaderCell>Rollback</CTableHeaderCell>
                  <CTableHeaderCell>Actions</CTableHeaderCell>
                </CTableRow>
              </CTableHead>
              <CTableBody>
                {selectedRuns.length === 0 ? (
                  <CTableRow>
                    <CTableDataCell colSpan={7} className="text-center text-muted">
                      No runs yet
                    </CTableDataCell>
                  </CTableRow>
                ) : (
                  selectedRuns.map((run) => (
                    <CTableRow key={run.id}>
                      <CTableDataCell>{run.run_number}</CTableDataCell>
                      <CTableDataCell>
                        <CBadge color={STATUS_COLORS[run.status] || "secondary"}>
                          {run.status}
                        </CBadge>
                      </CTableDataCell>
                      <CTableDataCell>
                        {run.started_at
                          ? new Date(run.started_at).toLocaleString()
                          : "—"}
                      </CTableDataCell>
                      <CTableDataCell>
                        {run.duration_ms
                          ? `${(run.duration_ms / 1000).toFixed(1)}s`
                          : "—"}
                      </CTableDataCell>
                      <CTableDataCell>{run.triggered_by || "—"}</CTableDataCell>
                      <CTableDataCell>{run.rollback_status || "N/A"}</CTableDataCell>
                      <CTableDataCell>
                        <CButton
                          color="info"
                          size="sm"
                          onClick={() => handleViewRunDetail(run.id)}
                        >
                          Details
                        </CButton>
                      </CTableDataCell>
                    </CTableRow>
                  ))
                )}
              </CTableBody>
            </CTable>
          )}
        </CModalBody>
      </CModal>

      {/* ================================================================ */}
      {/* RESULTS MODAL */}
      {/* ================================================================ */}
      <CModal
        visible={showResultsModal}
        onClose={() => setShowResultsModal(false)}
        size="xl"
      >
        <CModalHeader>
          <CModalTitle>
            Experiment Results:{" "}
            {selectedExperiment?.name || selectedExperiment?.experiment?.name}
          </CModalTitle>
        </CModalHeader>
        <CModalBody>
          {selectedResults.length === 0 ? (
            <CCallout color="info">
              No results yet. Run the experiment to generate results.
            </CCallout>
          ) : (
            selectedResults.map((result, i) => (
              <CCard key={i} className="mb-3">
                <CCardHeader className="d-flex justify-content-between">
                  <span>
                    Resource: <code>{result.resource_id}</code>
                  </span>
                  <CBadge
                    color={SEVERITY_COLORS[result.impact_severity] || "secondary"}
                  >
                    Impact: {result.impact_severity}
                  </CBadge>
                </CCardHeader>
                <CCardBody>
                  {result.impact_summary && (
                    <p>
                      <strong>Summary:</strong> {result.impact_summary}
                    </p>
                  )}
                  <CRow>
                    <CCol md={4}>
                      <h6>Baseline Metrics</h6>
                      <pre className="small bg-light p-2 rounded">
                        {JSON.stringify(result.baseline_metrics, null, 2)}
                      </pre>
                    </CCol>
                    <CCol md={4}>
                      <h6>During Experiment</h6>
                      <pre className="small bg-light p-2 rounded">
                        {JSON.stringify(result.during_metrics, null, 2)}
                      </pre>
                    </CCol>
                    <CCol md={4}>
                      <h6>Recovery Metrics</h6>
                      <pre className="small bg-light p-2 rounded">
                        {JSON.stringify(result.recovery_metrics, null, 2)}
                      </pre>
                    </CCol>
                  </CRow>
                  {result.recovery_time_ms && (
                    <div className="mt-2">
                      <strong>Recovery Time:</strong>{" "}
                      {(result.recovery_time_ms / 1000).toFixed(1)}s
                    </div>
                  )}
                  {result.observations && result.observations.length > 0 && (
                    <div className="mt-2">
                      <strong>Observations:</strong>
                      <pre className="small bg-light p-2 rounded mt-1">
                        {JSON.stringify(result.observations, null, 2)}
                      </pre>
                    </div>
                  )}
                </CCardBody>
              </CCard>
            ))
          )}
        </CModalBody>
      </CModal>
    </>
  );
};

export default Chaos;
