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
  CNav,
  CTabContent,
  CTabPane,
  CModal,
  CModalHeader,
  CModalTitle,
  CModalBody,
  CModalFooter,
  CForm,
  CFormInput,
  CFormLabel,
  CFormSelect,
  CAlert,
  CSpinner,
  CBadge,
  CProgress,
  CProgressBar,
  CNavbar,
  CContainer,
  CNavbarNav,
  CNavItem,
  CNavLink,
  CDropdown,
  CDropdownToggle,
  CDropdownMenu,
  CDropdownItem,
  CAccordion,
  CAccordionItem,
  CAccordionHeader,
  CAccordionBody
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import ReactMarkdown from "react-markdown";
import { CChart } from "@coreui/react-chartjs";
import api from "../services/api";
import DatabaseOverview from "../components/database/DatabaseOverview";
import PerformanceAnalysis from "../components/database/PerformanceAnalysis";
import QueryTool from "../components/database/QueryTool";
import SchemaExplorer from "../components/database/SchemaExplorer";
import DatabaseMonitoring from "../components/database/DatabaseMonitoring";
import MySqlTriage from "../components/database/MySqlTriage";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";

const DatabaseManagement = () => {
  // State management
  const [connections, setConnections] = useState([]);
  const [selectedConnection, setSelectedConnection] = useState(null);
  const [activeView, setActiveView] = useState("overview");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Connection modal state
  const [showConnectionModal, setShowConnectionModal] = useState(false);
  const [editingConnection, setEditingConnection] = useState(null);
  const [connectionForm, setConnectionForm] = useState({
    name: "",
    connection_type: "mysql",
    host: "",
    port: 3306,
    username: "",
    password: "",
    database_name: "",
    ssl_mode: "disable"
  });

  // Analysis state
  const [analysisResult, setAnalysisResult] = useState(null);
  const [analysisLoading, setAnalysisLoading] = useState(false);
  const [queryResult, setQueryResult] = useState(null);
  const [currentQuery, setCurrentQuery] = useState("");
  const [queryHistory, setQueryHistory] = useState([]);

  // Performance analysis state
  const [performanceMetrics, setPerformanceMetrics] = useState(null);
  const [analysisWorkflow, setAnalysisWorkflow] = useState("performance");

  // Grid refs
  const connectionsGridRef = useRef();
  const queryResultGridRef = useRef();

  // Analysis workflows for multi-lens perspective
  const ANALYSIS_WORKFLOWS = [
    { id: "performance", name: "Performance Analysis", icon: "🚀", description: "Overall database performance metrics" },
    { id: "cpu", name: "CPU Analysis", icon: "⚡", description: "CPU utilization and query optimization" },
    { id: "memory", name: "Memory Analysis", icon: "🧠", description: "Buffer pool and memory usage analysis" },
    { id: "disk", name: "Disk I/O Analysis", icon: "💾", description: "Storage and I/O performance metrics" },
    { id: "network", name: "Network Analysis", icon: "🌐", description: "Connection and network metrics" },
    { id: "buffer", name: "Buffer Analysis", icon: "📊", description: "Buffer pool hit ratios and efficiency" },
    { id: "slow-queries", name: "Slow Queries", icon: "🐌", description: "Identify and optimize slow queries" },
    { id: "index", name: "Index Analysis", icon: "📇", description: "Index usage and optimization" }
  ];

  // Load connections on component mount
  useEffect(() => {
    fetchConnections();
  }, []);

  const fetchConnections = async () => {
    try {
      setLoading(true);
      const response = await api.get("/api/databases");
      setConnections(response.data || []);
    } catch (err) {
      setError("Failed to fetch database connections: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleCreateConnection = async (e) => {
    e.preventDefault();
    try {
      setLoading(true);
      if (editingConnection) {
        await api.put(`/api/databases/${editingConnection.id}`, connectionForm);
        setSuccess("Connection updated successfully");
      } else {
        await api.post("/api/databases", connectionForm);
        setSuccess("Connection created successfully");
      }
      setShowConnectionModal(false);
      setEditingConnection(null);
      resetConnectionForm();
      await fetchConnections();
    } catch (err) {
      setError("Failed to save connection: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleEditConnection = (connection) => {
    setEditingConnection(connection);
    setConnectionForm({
      name: connection.name,
      connection_type: connection.connection_type,
      host: connection.host,
      port: connection.port,
      username: connection.username,
      password: "", // Don't populate password for security
      database_name: connection.database_name || "",
      ssl_mode: connection.ssl_mode || "disable"
    });
    setShowConnectionModal(true);
  };

  const handleDeleteConnection = async (connectionId) => {
    if (!window.confirm("Are you sure you want to delete this connection?")) return;

    try {
      await api.delete(`/api/databases/${connectionId}`);
      setSuccess("Connection deleted successfully");
      await fetchConnections();
      if (selectedConnection?.id === connectionId) {
        setSelectedConnection(null);
      }
    } catch (err) {
      setError("Failed to delete connection: " + (err.response?.data?.message || err.message));
    }
  };

  const handleTestConnection = async (connectionId) => {
    try {
      setLoading(true);
      const response = await api.post(`/api/databases/${connectionId}/test`);
      if (response.data.success) {
        setSuccess(`Connection test successful! Latency: ${response.data.latency_ms || 'N/A'}ms`);
      } else {
        setError("Connection test failed: " + response.data.message);
      }
    } catch (err) {
      setError("Connection test failed: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleAnalyzeDatabase = async (connectionId, workflow = "performance") => {
    try {
      setAnalysisLoading(true);
      const response = await api.get(`/api/databases/${connectionId}/analyze`);
      setAnalysisResult(response.data);
      setPerformanceMetrics(response.data.performance_metrics);
    } catch (err) {
      setError("Analysis failed: " + (err.response?.data?.message || err.message));
    } finally {
      setAnalysisLoading(false);
    }
  };

  const executeQuery = async () => {
    if (!selectedConnection || !currentQuery.trim()) return;

    try {
      setLoading(true);
      const response = await api.post(`/api/databases/${selectedConnection.id}/query`, {
        connection_id: selectedConnection.id,
        query: currentQuery.trim(),
        explain: activeView === "query" && currentQuery.toLowerCase().startsWith("select")
      });

      setQueryResult(response.data);
      setQueryHistory(prev => [...prev.slice(-9), {
        query: currentQuery,
        timestamp: new Date().toLocaleString(),
        execution_time: response.data.execution_time_ms,
        rows: response.data.row_count
      }]);
    } catch (err) {
      console.error('Query execution error:', err);  // Debug log
      setError("Query execution failed: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const resetConnectionForm = () => {
    setConnectionForm({
      name: "",
      connection_type: "mysql",
      host: "",
      port: 3306,
      username: "",
      password: "",
      database_name: "",
      ssl_mode: "disable"
    });
  };

  // Column definitions for connections grid
  const connectionColumns = [
    {
      headerName: "Name",
      field: "name",
      flex: 1,
      cellRenderer: (params) => (
        <div className="d-flex align-items-center">
          <span className="me-2">{getDatabaseIcon(params.data.connection_type)}</span>
          <strong>{params.value}</strong>
        </div>
      )
    },
    {
      headerName: "Type",
      field: "connection_type",
      width: 100,
      cellRenderer: (params) => (
        <CBadge color={getDatabaseTypeColor(params.value)}>
          {params.value.toUpperCase()}
        </CBadge>
      )
    },
    {
      headerName: "Host",
      field: "host",
      width: 150
    },
    {
      headerName: "Port",
      field: "port",
      width: 80
    },
    {
      headerName: "Database",
      field: "database_name",
      width: 120
    },
    {
      headerName: "Status",
      field: "connection_status",
      width: 100,
      cellRenderer: (params) => (
        <CBadge color={params.value === "active" ? "success" : "secondary"}>
          {params.value || "unknown"}
        </CBadge>
      )
    },
    {
      headerName: "Actions",
      field: "actions",
      width: 200,
      cellRenderer: (params) => (
        <div className="d-flex gap-1">
          <CButton size="sm" color="primary" variant="outline"
            onClick={() => setSelectedConnection(params.data)}>
            Connect
          </CButton>
          <CButton size="sm" color="info" variant="outline"
            onClick={() => handleTestConnection(params.data.id)}>
            Test
          </CButton>
          <CButton size="sm" color="warning" variant="outline"
            onClick={() => handleEditConnection(params.data)}>
            Edit
          </CButton>
          <CButton size="sm" color="danger" variant="outline"
            onClick={() => handleDeleteConnection(params.data.id)}>
            Delete
          </CButton>
        </div>
      )
    }
  ];

  // Helper functions
  const getDatabaseIcon = (type) => {
    const icons = {
      mysql: "🐬",
      postgresql: "🐘",
      postgres: "🐘",
      redis: "🔴",
      opensearch: "🔍"
    };
    return icons[type] || "💾";
  };

  const getDatabaseTypeColor = (type) => {
    const colors = {
      mysql: "primary",
      postgresql: "info",
      postgres: "info",
      redis: "danger",
      opensearch: "warning"
    };
    return colors[type] || "secondary";
  };

  return (
    <div style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
      {/* Header */}
      <CNavbar expand="lg" colorScheme="light" className="bg-light border-bottom">
        <CContainer fluid>
          <h4 className="mb-0">📊 Database Management Center</h4>
          <CNavbarNav className="ms-auto">
            <CNavItem>
              <CButton color="primary" onClick={() => setShowConnectionModal(true)}>
                ➕ Add Connection
              </CButton>
            </CNavItem>
          </CNavbarNav>
        </CContainer>
      </CNavbar>

      {/* Alerts */}
      {error && (
        <CAlert color="danger" dismissible onDismiss={() => setError(null)}>
          {error}
        </CAlert>
      )}
      {success && (
        <CAlert color="success" dismissible onDismiss={() => setSuccess(null)}>
          {success}
        </CAlert>
      )}

      <div style={{ flex: 1, display: "flex" }}>
        {/* Sidebar - Connection List */}
        <div style={{ width: "350px", borderRight: "1px solid #dee2e6", backgroundColor: "#f8f9fa" }}>
          <CCard className="h-100 border-0 rounded-0">
            <CCardHeader className="bg-primary text-white">
              <h6 className="mb-0">Database Connections</h6>
            </CCardHeader>
            <CCardBody className="p-0">
              <div className="ag-theme-alpine" style={{ height: "100%", width: "100%" }}>
                <AgGridReact
                  ref={connectionsGridRef}
                  columnDefs={connectionColumns.slice(0, 3)} // Show only basic info in sidebar
                  rowData={connections}
                  defaultColDef={{ resizable: true, sortable: true }}
                  onRowClicked={(event) => setSelectedConnection(event.data)}
                  rowHeight={50}
                  headerHeight={35}
                />
              </div>
            </CCardBody>
          </CCard>
        </div>

        {/* Main Content Area */}
        <div style={{ flex: 1, display: "flex", flexDirection: "column" }}>
          {selectedConnection ? (
            <DatabaseWorkbench
              connection={selectedConnection}
              analysisWorkflows={ANALYSIS_WORKFLOWS}
              onAnalyze={handleAnalyzeDatabase}
              analysisResult={analysisResult}
              analysisLoading={analysisLoading}
              performanceMetrics={performanceMetrics}
              queryResult={queryResult}
              currentQuery={currentQuery}
              setCurrentQuery={setCurrentQuery}
              executeQuery={executeQuery}
              queryHistory={queryHistory}
              loading={loading}
            />
          ) : (
            <div className="d-flex align-items-center justify-content-center h-100">
              <div className="text-center">
                <h5 className="text-muted">Select a database connection to begin</h5>
                <p className="text-muted">Choose a connection from the sidebar or create a new one</p>
                <CButton color="primary" onClick={() => setShowConnectionModal(true)}>
                  ➕ Create New Connection
                </CButton>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Connection Modal */}
      <CModal visible={showConnectionModal} onClose={() => {
        setShowConnectionModal(false);
        setEditingConnection(null);
        resetConnectionForm();
      }} size="lg">
        <CModalHeader>
          <CModalTitle>
            {editingConnection ? "Edit Connection" : "Create New Connection"}
          </CModalTitle>
        </CModalHeader>
        <CForm onSubmit={handleCreateConnection}>
          <CModalBody>
            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel htmlFor="name">Connection Name</CFormLabel>
                <CFormInput
                  id="name"
                  value={connectionForm.name}
                  onChange={(e) => setConnectionForm({ ...connectionForm, name: e.target.value })}
                  required
                />
              </CCol>
              <CCol md={6}>
                <CFormLabel htmlFor="type">Database Type</CFormLabel>
                <CFormSelect
                  id="type"
                  value={connectionForm.connection_type}
                  onChange={(e) => {
                    const type = e.target.value;
                    const defaultPorts = { mysql: 3306, postgresql: 5432, postgres: 5432, redis: 6379, opensearch: 9200 };
                    setConnectionForm({
                      ...connectionForm,
                      connection_type: type,
                      port: defaultPorts[type] || 3306
                    });
                  }}
                >
                  <option value="mysql">MySQL</option>
                  <option value="postgresql">PostgreSQL</option>
                  <option value="redis">Redis</option>
                  <option value="opensearch">OpenSearch</option>
                </CFormSelect>
              </CCol>
            </CRow>
            <CRow className="mb-3">
              <CCol md={8}>
                <CFormLabel htmlFor="host">Host</CFormLabel>
                <CFormInput
                  id="host"
                  value={connectionForm.host}
                  onChange={(e) => setConnectionForm({ ...connectionForm, host: e.target.value })}
                  placeholder="localhost"
                  required
                />
              </CCol>
              <CCol md={4}>
                <CFormLabel htmlFor="port">Port</CFormLabel>
                <CFormInput
                  type="number"
                  id="port"
                  value={connectionForm.port}
                  onChange={(e) => setConnectionForm({ ...connectionForm, port: parseInt(e.target.value) })}
                  required
                />
              </CCol>
            </CRow>
            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel htmlFor="username">Username</CFormLabel>
                <CFormInput
                  id="username"
                  value={connectionForm.username}
                  onChange={(e) => setConnectionForm({ ...connectionForm, username: e.target.value })}
                />
              </CCol>
              <CCol md={6}>
                <CFormLabel htmlFor="password">Password</CFormLabel>
                <CFormInput
                  type="password"
                  id="password"
                  value={connectionForm.password}
                  onChange={(e) => setConnectionForm({ ...connectionForm, password: e.target.value })}
                />
              </CCol>
            </CRow>
            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel htmlFor="database">Database Name</CFormLabel>
                <CFormInput
                  id="database"
                  value={connectionForm.database_name}
                  onChange={(e) => setConnectionForm({ ...connectionForm, database_name: e.target.value })}
                />
              </CCol>
              <CCol md={6}>
                <CFormLabel htmlFor="ssl">SSL Mode</CFormLabel>
                <CFormSelect
                  id="ssl"
                  value={connectionForm.ssl_mode}
                  onChange={(e) => setConnectionForm({ ...connectionForm, ssl_mode: e.target.value })}
                >
                  <option value="disable">Disable</option>
                  <option value="require">Require</option>
                  <option value="prefer">Prefer</option>
                </CFormSelect>
              </CCol>
            </CRow>
          </CModalBody>
          <CModalFooter>
            <CButton color="secondary" onClick={() => setShowConnectionModal(false)}>
              Cancel
            </CButton>
            <CButton color="primary" type="submit" disabled={loading}>
              {loading ? <CSpinner size="sm" /> : (editingConnection ? "Update" : "Create")}
            </CButton>
          </CModalFooter>
        </CForm>
      </CModal>
    </div>
  );
};

// Database Workbench Component
const DatabaseWorkbench = ({
  connection,
  analysisWorkflows,
  onAnalyze,
  analysisResult,
  analysisLoading,
  performanceMetrics,
  queryResult,
  currentQuery,
  setCurrentQuery,
  executeQuery,
  queryHistory,
  loading
}) => {
  const [activeTab, setActiveTab] = useState("overview");
  const [selectedWorkflow, setSelectedWorkflow] = useState("performance");

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column" }}>
      {/* Connection Header */}
      <div className="bg-light border-bottom p-3">
        <div className="d-flex align-items-center justify-content-between">
          <div className="d-flex align-items-center">
            <span className="me-2 fs-4">{getDatabaseIcon(connection.connection_type)}</span>
            <div>
              <h5 className="mb-0">{connection.name}</h5>
              <small className="text-muted">
                {connection.connection_type.toUpperCase()} • {connection.host}:{connection.port}
              </small>
            </div>
          </div>
          <div className="d-flex gap-2">
            <CButton size="sm" color="info" variant="outline"
              onClick={() => onAnalyze(connection.id, selectedWorkflow)}>
              🔍 Analyze
            </CButton>
            <CDropdown>
              <CDropdownToggle size="sm" color="primary" variant="outline">
                📊 {analysisWorkflows.find(w => w.id === selectedWorkflow)?.name}
              </CDropdownToggle>
              <CDropdownMenu>
                {analysisWorkflows.map(workflow => (
                  <CDropdownItem key={workflow.id} onClick={() => setSelectedWorkflow(workflow.id)}>
                    {workflow.icon} {workflow.name}
                  </CDropdownItem>
                ))}
              </CDropdownMenu>
            </CDropdown>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div>
        <CNav variant="pills" className="px-3 pt-3 bg-light">
          <CNavItem>
            <CNavLink
              active={activeTab === 'overview'}
              onClick={() => setActiveTab('overview')}
              href="#"
            >
              📈 Overview
            </CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink
              active={activeTab === 'performance'}
              onClick={() => setActiveTab('performance')}
              href="#"
            >
              🚀 Performance
            </CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink
              active={activeTab === 'query'}
              onClick={() => setActiveTab('query')}
              href="#"
            >
              🔍 Query Tool
            </CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink
              active={activeTab === 'schema'}
              onClick={() => setActiveTab('schema')}
              href="#"
            >
              📋 Schema
            </CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink
              active={activeTab === 'monitoring'}
              onClick={() => setActiveTab('monitoring')}
              href="#"
            >
              📊 Monitoring
            </CNavLink>
          </CNavItem>
          {connection.connection_type === 'mysql' && (
            <CNavItem>
              <CNavLink
                active={activeTab === 'triage'}
                onClick={() => setActiveTab('triage')}
                href="#"
              >
                🤖 AI Triage
              </CNavLink>
            </CNavItem>
          )}
        </CNav>

        <CTabContent style={{ flex: 1 }}>
          <CTabPane visible={activeTab === 'overview'} className="p-3">
            <DatabaseOverview
              connection={connection}
              analysisResult={analysisResult}
              performanceMetrics={performanceMetrics}
            />
          </CTabPane>

          <CTabPane visible={activeTab === 'performance'} className="p-3">
            <PerformanceAnalysis
              connection={connection}
              analysisWorkflows={analysisWorkflows}
              selectedWorkflow={selectedWorkflow}
              onWorkflowChange={setSelectedWorkflow}
              onAnalyze={onAnalyze}
              analysisResult={analysisResult}
              analysisLoading={analysisLoading}
              performanceMetrics={performanceMetrics}
            />
          </CTabPane>

          <CTabPane visible={activeTab === 'query'} className="p-3">
            <QueryTool
              connection={connection}
              currentQuery={currentQuery}
              setCurrentQuery={setCurrentQuery}
              executeQuery={executeQuery}
              queryResult={queryResult}
              queryHistory={queryHistory}
              loading={loading}
            />
          </CTabPane>

          <CTabPane visible={activeTab === 'schema'} className="p-3">
            <SchemaExplorer connection={connection} />
          </CTabPane>

          <CTabPane visible={activeTab === 'monitoring'} className="p-3">
            <DatabaseMonitoring
              connection={connection}
              performanceMetrics={performanceMetrics}
            />
          </CTabPane>

          {connection.connection_type === 'mysql' && (
            <CTabPane visible={activeTab === 'triage'} className="p-3">
              <MySqlTriage connection={connection} />
            </CTabPane>
          )}
        </CTabContent>
      </div>
    </div>
  );
};

// Helper function (moved outside component to avoid re-creation)
const getDatabaseIcon = (type) => {
  const icons = {
    mysql: "🐬",
    postgresql: "🐘",
    postgres: "🐘",
    redis: "🔴",
    opensearch: "🔍"
  };
  return icons[type] || "💾";
};

export default DatabaseManagement;
