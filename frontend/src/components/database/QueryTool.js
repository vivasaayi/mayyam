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
  CTabs,
  CTabList,
  CTab,
  CTabContent,
  CTabPanel
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
      // Add cellRenderer for better display of null/undefined values
      cellRenderer: (params) => {
        if (params.value === null || params.value === undefined) {
          return '<span class="text-muted">(null)</span>';
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

    console.log('Generated column definitions:', columnDefs);
    console.log('Row data:', rowData);

    return {
      columnDefs,
      rowData
    };
  };

  const gridData = formatQueryResult(queryResult);
  
  // Debug logging to see what data is being passed to AGGrid
  React.useEffect(() => {
    if (queryResult) {
      console.log('QueryResult:', queryResult);
      console.log('GridData:', gridData);
      console.log('Columns (expanded):', JSON.stringify(gridData?.columnDefs, null, 2));
      console.log('Rows (first 3, expanded):', JSON.stringify(gridData?.rowData?.slice(0, 3), null, 2));
      console.log('Row count:', gridData?.rowData?.length);
    }
  }, [queryResult, gridData]);

  return (
    <div style={{ height: "100%" }}>
      <CTabs activeTab={activeTab} onTabChange={setActiveTab}>
        <CTabList variant="pills" className="mb-3">
          <CTab value="editor">✏️ Query Editor</CTab>
          <CTab value="results">📋 Results</CTab>
          <CTab value="history">🕒 History</CTab>
          <CTab value="templates">📄 Query Templates</CTab>
        </CTabList>

        <CTabContent style={{ height: "calc(100% - 60px)" }}>
          <CTabPanel value="editor">
            <CRow style={{ height: "100%" }}>
              <CCol lg={8} style={{ height: "100%" }}>
                <CCard style={{ height: "100%" }}>
                  <CCardHeader className="d-flex justify-content-between align-items-center">
                    <strong>SQL Editor</strong>
                    <div className="d-flex gap-2">
                      <CFormSelect 
                        size="sm" 
                        style={{ width: "auto" }}
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
                        {loading ? <CSpinner size="sm" /> : "▶️ Execute"}
                      </CButton>
                    </div>
                  </CCardHeader>
                  <CCardBody style={{ height: "calc(100% - 60px)", padding: 0 }}>
                    <CFormTextarea
                      style={{ 
                        height: "100%", 
                        border: "none", 
                        resize: "none",
                        fontFamily: "Monaco, 'Courier New', monospace"
                      }}
                      value={currentQuery}
                      onChange={(e) => setCurrentQuery(e.target.value)}
                      placeholder="Enter your SQL query here..."
                    />
                  </CCardBody>
                </CCard>
              </CCol>
              <CCol lg={4} style={{ height: "100%" }}>
                <CCard style={{ height: "100%" }}>
                  <CCardHeader>
                    <strong>Quick Actions</strong>
                  </CCardHeader>
                  <CCardBody style={{ height: "calc(100% - 60px)", overflowY: "auto" }}>
                    <div className="mb-3">
                      <h6>Quick Actions</h6>
                      {templatesLoading ? (
                        <div className="text-center my-2">
                          <CSpinner size="sm" />
                        </div>
                      ) : templatesError ? (
                        <CAlert color="warning" className="p-2 small">
                          {templatesError}
                        </CAlert>
                      ) : templates.length === 0 ? (
                        <CAlert color="info" className="p-2 small">
                          No templates available. Create templates in the "Query Templates" tab.
                        </CAlert>
                      ) : (
                        templates
                          .filter(template => template.is_favorite)
                          .slice(0, 5)
                          .map((template, index) => (
                            <CButton
                              key={template.id}
                              variant="outline"
                              color="info"
                              size="sm"
                              className="me-2 mb-2 d-block w-100 text-start"
                              onClick={() => loadCommonQuery(template)}
                            >
                              {template.name}
                            </CButton>
                          ))
                      )}
                    </div>
                    
                    {queryResult && (
                      <div className="mb-3">
                        <h6>Last Result Summary</h6>
                        <div className="small">
                          <div>Rows: {queryResult.row_count}</div>
                          <div>Execution Time: {queryResult.execution_time_ms}ms</div>
                          <div>Columns: {queryResult.columns.length}</div>
                        </div>
                      </div>
                    )}

                    {queryHistory.length > 0 && (
                      <div>
                        <h6>Recent Queries</h6>
                        {queryHistory.slice(-3).reverse().map((item, index) => (
                          <div key={index} className="border p-2 mb-2 small">
                            <div className="text-truncate" style={{ maxWidth: "200px" }}>
                              {item.query}
                            </div>
                            <div className="text-muted">
                              {item.timestamp} • {item.execution_time}ms • {item.rows} rows
                            </div>
                            <CButton
                              size="sm"
                              variant="outline"
                              color="primary"
                              onClick={() => setCurrentQuery(item.query)}
                            >
                              Load
                            </CButton>
                          </div>
                        ))}
                      </div>
                    )}
                  </CCardBody>
                </CCard>
              </CCol>
            </CRow>
          </CTabPanel>

          <CTabPanel value="results">
            <CCard style={{ height: "100%" }}>
              <CCardHeader className="d-flex justify-content-between align-items-center">
                <strong>Query Results</strong>
                {queryResult && (
                  <div className="d-flex gap-3">
                    <CBadge color="info">{queryResult.row_count} rows</CBadge>
                    <CBadge color="success">{queryResult.execution_time_ms}ms</CBadge>
                    <CBadge color="primary">{queryResult.columns.length} columns</CBadge>
                  </div>
                )}
              </CCardHeader>
              <CCardBody style={{ height: "calc(100% - 60px)", padding: 0 }}>
                {queryResult ? (
                  gridData && gridData.columnDefs && gridData.rowData ? (
                    <div className="ag-theme-alpine" style={{ 
                      height: "calc(100vh - 250px)", 
                      width: "100%",
                      border: "1px solid #ddd",
                      minHeight: "400px" // Ensure grid has a minimum height
                    }}>
                      <AgGridReact
                        ref={gridRef}
                        columnDefs={gridData.columnDefs}
                        rowData={gridData.rowData}
                        defaultColDef={{
                          resizable: true,
                          sortable: true,
                          filter: true,
                          minWidth: 100,
                          flex: 1 // Make columns flexible
                        }}
                        pagination={true}
                        paginationPageSize={50}
                        suppressRowClickSelection={true}
                        animateRows={true}
                        domLayout="normal" // Changed from autoHeight to normal
                        onGridReady={(params) => {
                          // Fit columns when grid is ready
                          setTimeout(() => {
                            params.api.sizeColumnsToFit();
                          }, 100);
                        }}
                        onFirstDataRendered={(params) => {
                          // Autosize columns after data is loaded
                          params.columnApi.autoSizeAllColumns();
                        }}
                      />
                    </div>
                  ) : (
                    <div className="p-3">
                      <CAlert color="warning">
                        <h6>No data to display</h6>
                        <p>Query executed but returned no displayable data or the data format is unexpected.</p>
                        <small>
                          Columns: {queryResult.columns?.length || 0} | 
                          Rows: {queryResult.rows?.length || 0}
                        </small>
                      </CAlert>
                      <div className="mt-3">
                        <h6>Raw Query Result</h6>
                        <pre className="bg-light p-2 small" style={{ maxHeight: "300px", overflow: "auto" }}>
                          {JSON.stringify(queryResult, null, 2)}
                        </pre>
                      </div>
                      {queryResult.rows && queryResult.rows.length > 0 && (
                        <div className="mt-3">
                          <h6>Data Preview (First Row)</h6>
                          <pre className="bg-light p-2 small" style={{ maxHeight: "200px", overflow: "auto" }}>
                            {JSON.stringify(queryResult.rows[0], null, 2)}
                          </pre>
                        </div>
                      )}
                    </div>
                  )
                ) : (
                  <div className="d-flex align-items-center justify-content-center h-100 text-muted">
                    Execute a query to see results
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CTabPanel>

          <CTabPanel value="history">
            <CCard style={{ height: "100%" }}>
              <CCardHeader>
                <strong>Query History</strong>
              </CCardHeader>
              <CCardBody style={{ height: "calc(100% - 60px)", overflowY: "auto" }}>
                {queryHistory.length > 0 ? (
                  <div>
                    {queryHistory.slice().reverse().map((item, index) => (
                      <CCard key={index} className="mb-3">
                        <CCardBody>
                          <div className="d-flex justify-content-between align-items-start mb-2">
                            <div>
                              <CBadge color="info" className="me-2">{item.timestamp}</CBadge>
                              <CBadge color="success" className="me-2">{item.execution_time}ms</CBadge>
                              <CBadge color="primary">{item.rows} rows</CBadge>
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
                          </div>
                          <pre className="bg-light p-2 small" style={{ whiteSpace: "pre-wrap" }}>
                            {item.query}
                          </pre>
                        </CCardBody>
                      </CCard>
                    ))}
                  </div>
                ) : (
                  <div className="text-center text-muted">
                    No query history available
                  </div>
                )}
              </CCardBody>
            </CCard>
          </CTabPanel>

          <CTabPanel value="templates">
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
              <CCardBody style={{ height: "calc(100% - 60px)", overflowY: "auto" }}>
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
                    <h6>Favorite Templates (Quick Access)</h6>
                    <div className="mb-4">
                      {templates.filter(t => t.is_favorite).map(template => (
                        <CButton
                          key={template.id}
                          color="light"
                          className="m-1"
                          onClick={() => {
                            loadCommonQuery(template);
                            setActiveTab("editor");
                          }}
                        >
                          {template.name}
                        </CButton>
                      ))}
                      {templates.filter(t => t.is_favorite).length === 0 && (
                        <p className="text-muted small">No favorite templates. Mark templates as favorites in the Template Manager.</p>
                      )}
                    </div>
                    
                    <h6>All Templates</h6>
                    <div className="template-grid">
                      {templates.map(template => (
                        <CCard key={template.id} className="mb-3">
                          <CCardHeader className="py-2 d-flex justify-content-between align-items-center">
                            <strong>{template.name}</strong>
                            {template.is_favorite && <CBadge color="info">Favorite</CBadge>}
                          </CCardHeader>
                          <CCardBody className="p-3">
                            {template.description && (
                              <p className="text-muted small mb-2">{template.description}</p>
                            )}
                            <div className="code-preview mb-2">
                              <pre className="p-2 bg-light">{template.query}</pre>
                            </div>
                            <CButton 
                              size="sm" 
                              color="primary"
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
          </CTabPanel>
        </CTabContent>
      </CTabs>
    </div>
  );
};

export default QueryTool;
