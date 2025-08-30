import React, { useState, useRef, useEffect } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CForm,
  CFormTextarea,
  CFormSelect,
  CAlert,
  CSpinner,
  CBadge,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import ReactMarkdown from "react-markdown";
import QueryTemplateService from "../../services/queryTemplateService";
import QueryTemplateManager from "./QueryTemplateManager";
import "../../styles/QueryTemplates.css"; // Import the CSS file

const QueryTool = ({ 
  connection, 
  currentQuery, 
  setCurrentQuery, 
  executeQuery, 
  queryResult, 
  queryHistory, 
  loading 
}) => {
  const [activeTab, setActiveTab] = useState("editor");
  const [queryOptions, setQueryOptions] = useState({
    explain: false,
    analyze: false,
    limit: 1000
  });
  const [templates, setTemplates] = useState([]);
  const [templatesLoading, setTemplatesLoading] = useState(true);
  const [templatesError, setTemplatesError] = useState(null);

  const gridRef = useRef();

  // Load query templates when connection changes
  useEffect(() => {
    if (connection) {
      loadTemplates();
    }
  }, [connection]);

  // Load templates from the backend
  const loadTemplates = async () => {
    try {
      setTemplatesLoading(true);
      setTemplatesError(null);
      const data = await QueryTemplateService.getTemplatesByType(connection.connection_type);
      setTemplates(data);
    } catch (err) {
      console.error("Failed to load templates:", err);
      setTemplatesError("Failed to load query templates");
    } finally {
      setTemplatesLoading(false);
    }
  };

  const executeCurrentQuery = async () => {
    if (!currentQuery.trim()) return;
    await executeQuery();
  };

  // Load a query from a template
  const loadCommonQuery = (template) => {
    setCurrentQuery(template.query);
  };

  const formatQueryResult = (result) => {
    if (!result || !result.columns || !result.rows) {
      console.warn('Invalid query result:', result);
      return null;
    }

    if (result.columns.length === 0) {
      console.warn('No columns in query result');
      return null;
    }

    // Create column definitions
    const columnDefs = result.columns.map(col => ({
      headerName: col,
      field: col,
      resizable: true,
      sortable: true,
      filter: true,
      minWidth: 150,
      flex: 1,
      // Handle null/undefined values properly for display
      valueFormatter: (params) => {
        if (params.value === null || params.value === undefined) {
          return '(null)';
        }
        return params.value;
      }
    }));

    // Ensure all rows have all fields defined
    const rowData = result.rows.map(row => {
      const normalizedRow = {};
      // Initialize all fields with null
      result.columns.forEach(col => {
        normalizedRow[col] = row[col] !== undefined ? row[col] : null;
      });
      return normalizedRow;
    });

    return {
      columnDefs,
      rowData
    };
  };

  const gridData = formatQueryResult(queryResult);

  return (
    <div style={{ height: "100%" }} className="query-tool-container">
      <CNav variant="tabs" className="query-tabs">
        <CNavItem>
          <CNavLink 
            active={activeTab === 'editor'}
            onClick={() => setActiveTab('editor')}
            href="#"
            className={activeTab === 'editor' ? 'active' : ''}
          >
            ✏️ Query Editor
          </CNavLink>
        </CNavItem>
        <CNavItem>
          <CNavLink 
            active={activeTab === 'results'}
            onClick={() => setActiveTab('results')}
            href="#"
            className={activeTab === 'results' ? 'active' : ''}
          >
            📋 Results
          </CNavLink>
        </CNavItem>
        <CNavItem>
          <CNavLink 
            active={activeTab === 'history'}
            onClick={() => setActiveTab('history')}
            href="#"
            className={activeTab === 'history' ? 'active' : ''}
          >
            🕒 History
          </CNavLink>
        </CNavItem>
        <CNavItem>
          <CNavLink 
            active={activeTab === 'templates'}
            onClick={() => setActiveTab('templates')}
            href="#"
            className={activeTab === 'templates' ? 'active' : ''}
          >
            📄 Templates
          </CNavLink>
        </CNavItem>
      </CNav>

      <CTabContent style={{ height: "calc(100vh - 150px)" }}>
          <CTabPane visible={activeTab === 'editor'}>
            <CCard style={{ height: "100%" }}>
              <CCardHeader className="d-flex justify-content-between align-items-center py-2">
                <div className="d-flex align-items-center">
                  <strong>SQL Query</strong>
                  {connection && (
                    <span className="ms-2 text-muted small">
                      {connection.name} ({connection.connection_type})
                    </span>
                  )}
                </div>
                <div className="d-flex gap-2 align-items-center">
                  <CFormSelect 
                    size="sm" 
                    style={{ width: "120px" }}
                    value={queryOptions.limit}
                    onChange={(e) => setQueryOptions({...queryOptions, limit: parseInt(e.target.value)})}
                  >
                    <option value={100}>Limit 100</option>
                    <option value={500}>Limit 500</option>
                    <option value={1000}>Limit 1000</option>
                    <option value={5000}>Limit 5000</option>
                  </CFormSelect>
                  <CButton 
                    color="primary" 
                    size="sm"
                    onClick={executeCurrentQuery}
                    disabled={loading || !currentQuery.trim()}
                  >
                    {loading ? <CSpinner size="sm" /> : "Execute"}
                  </CButton>
                </div>
              </CCardHeader>
              <CCardBody className="p-0" style={{ height: "calc(100vh - 200px)" }}>
                <CFormTextarea
                  className="sql-editor"
                  style={{ 
                    height: "100%", 
                    width: "100%",
                    border: "none", 
                    resize: "none",
                    fontFamily: "Monaco, 'Courier New', monospace",
                    padding: "12px",
                    fontSize: "14px",
                    lineHeight: "1.5",
                    minHeight: "400px",
                    overflowY: "auto"
                  }}
                  rows={25}
                  value={currentQuery}
                  onChange={(e) => setCurrentQuery(e.target.value)}
                  placeholder="Enter your SQL query here..."
                />
              </CCardBody>
            </CCard>
          </CTabPane>

          <CTabPane visible={activeTab === 'results'}>
            <CCard style={{ height: "100%" }}>
              <CCardHeader className="py-2 d-flex justify-content-between align-items-center">
                <strong>Query Results</strong>
                {queryResult && (
                  <div className="d-flex gap-2">
                    <CBadge color="info" className="p-1">{queryResult.row_count} rows</CBadge>
                    <CBadge color="success" className="p-1">{queryResult.execution_time_ms}ms</CBadge>
                    <CBadge color="primary" className="p-1">{queryResult.columns.length} columns</CBadge>
                  </div>
                )}
              </CCardHeader>
              <CCardBody className="p-0" style={{ height: "calc(100vh - 200px)" }}>
                {queryResult ? (
                  gridData && gridData.columnDefs && gridData.rowData ? (
                    <div className="ag-theme-alpine" style={{ 
                      height: "100%", 
                      width: "100%",
                      minHeight: "400px"
                    }}>
                      <AgGridReact
                        ref={gridRef}
                        columnDefs={gridData.columnDefs}
                        rowData={gridData.rowData}
                        defaultColDef={{
                          resizable: true,
                          sortable: true,
                          filter: true,
                          minWidth: 120
                        }}
                        pagination={true}
                        paginationPageSize={50}
                        suppressRowClickSelection={true}
                        animateRows={true}
                        onGridReady={(params) => {
                          setTimeout(() => params.api.sizeColumnsToFit(), 100);
                        }}
                      />
                    </div>
                  ) : (
                    <div className="p-3">
                      <CAlert color="warning">
                        <h6>No data to display</h6>
                        <p>Query executed but returned no displayable data or the data format is unexpected.</p>
                      </CAlert>
                    </div>
                  )
                ) : (
                  <div className="d-flex align-items-center justify-content-center h-100 text-muted">
                    Execute a query to see results
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CTabPane>

          <CTabPane visible={activeTab === 'history'}>
            <CCard style={{ height: "100%" }}>
              <CCardHeader className="py-2">
                <strong>Query History</strong>
              </CCardHeader>
              <CCardBody className="p-0" style={{ height: "calc(100vh - 200px)", overflowY: "auto" }}>
                {queryHistory.length > 0 ? (
                  <div className="p-2">
                    {queryHistory.slice().reverse().map((item, index) => (
                      <CCard key={index} className="mb-2 history-item">
                        <CCardHeader className="py-2 d-flex justify-content-between align-items-center">
                          <div className="d-flex gap-2">
                            <small>{item.timestamp}</small>
                            <CBadge color="success" className="p-1">{item.execution_time}ms</CBadge>
                            <CBadge color="primary" className="p-1">{item.rows} rows</CBadge>
                          </div>
                          <CButton
                            size="sm"
                            color="primary"
                            variant="outline"
                            onClick={() => {
                              setCurrentQuery(item.query);
                              setActiveTab("editor");
                            }}
                          >
                            Load
                          </CButton>
                        </CCardHeader>
                        <CCardBody className="py-2">
                          <pre className="mb-0 code-block">{item.query}</pre>
                        </CCardBody>
                      </CCard>
                    ))}
                  </div>
                ) : (
                  <div className="text-center text-muted p-4">
                    No query history available
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CTabPane>
          
          <CTabPane visible={activeTab === 'templates'}>
            <CCard style={{ height: "100%" }}>
              <CCardHeader className="d-flex justify-content-between align-items-center">
                <strong>Query Templates - {connection.connection_type.toUpperCase()}</strong>
                <CButton 
                  color="primary" 
                  size="sm"
                  href="#/query-templates"
                  target="_blank"
                >
                  Open Template Manager
                </CButton>
              </CCardHeader>
              <CCardBody style={{ height: "calc(100vh - 200px)", overflowY: "auto" }}>
                {templatesLoading ? (
                  <div className="text-center p-3">
                    <CSpinner />
                    <p className="mt-2">Loading templates...</p>
                  </div>
                ) : templatesError ? (
                  <CAlert color="danger">{templatesError}</CAlert>
                ) : templates.length === 0 ? (
                  <CAlert color="info">
                    <p>No templates available for {connection.connection_type}.</p>
                    <p>
                      <a href="#/query-templates" target="_blank">Create templates in the Query Templates Manager</a>
                    </p>
                  </CAlert>
                ) : (
                  <div className="template-list">
                    {templates.filter(t => t.is_favorite).length > 0 && (
                      <div className="mb-4">
                        <h6 className="border-bottom pb-2">Favorite Templates (Quick Access)</h6>
                        <div className="d-flex flex-wrap gap-2 mb-3">
                          {templates.filter(t => t.is_favorite).map(template => (
                            <CButton
                              key={template.id}
                              color="info"
                              className="template-favorite-button"
                              onClick={() => {
                                loadCommonQuery(template);
                                setActiveTab("editor");
                              }}
                            >
                              {template.name}
                            </CButton>
                          ))}
                        </div>
                      </div>
                    )}
                    
                    <h6 className="border-bottom pb-2">All Templates</h6>
                    <div className="template-grid">
                      {templates.map(template => (
                        <CCard key={template.id} className="mb-3 template-card">
                          <CCardHeader className="py-2 d-flex justify-content-between align-items-center">
                            <div className="d-flex align-items-center">
                              <strong>{template.name}</strong>
                              {template.is_favorite && <span className="ms-2 text-warning">⭐</span>}
                            </div>
                            {template.category && (
                              <CBadge color="light" className="text-dark">{template.category}</CBadge>
                            )}
                          </CCardHeader>
                          <CCardBody className="py-2">
                            {template.description && (
                              <p className="text-muted small mb-2">{template.description}</p>
                            )}
                            <div className="code-preview mb-2">
                              <pre className="p-2 bg-light">{template.query}</pre>
                            </div>
                            <CButton 
                              size="sm" 
                              color="primary"
                              className="w-100"
                              onClick={() => {
                                loadCommonQuery(template);
                                setActiveTab("editor");
                              }}
                            >
                              Use This Query
                            </CButton>
                          </CCardBody>
                        </CCard>
                      ))}
                    </div>
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CTabPane>
        </CTabContent>
      </div>
    );
  };

export default QueryTool;
