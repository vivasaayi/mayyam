import React, { useState, useRef } from "react";
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

  const gridRef = useRef();

  const commonQueries = {
    mysql: [
      {
        name: "Show Tables",
        query: "SHOW TABLES;"
      },
      {
        name: "Show Process List", 
        query: "SHOW PROCESSLIST;"
      },
      {
        name: "Performance Schema - Top Queries by Execution Time",
        query: `SELECT 
  SUBSTRING(digest_text, 1, 50) AS query_snippet,
  count_star AS exec_count,
  avg_timer_wait/1000000000 AS avg_time_ms,
  sum_timer_wait/1000000000 AS total_time_ms
FROM performance_schema.events_statements_summary_by_digest 
ORDER BY avg_timer_wait DESC 
LIMIT 10;`
      },
      {
        name: "Performance Schema - Buffer Pool Hit Ratio",
        query: `SELECT 
  (SELECT VARIABLE_VALUE FROM performance_schema.global_status WHERE VARIABLE_NAME = 'Innodb_buffer_pool_read_requests') AS read_requests,
  (SELECT VARIABLE_VALUE FROM performance_schema.global_status WHERE VARIABLE_NAME = 'Innodb_buffer_pool_reads') AS disk_reads,
  ROUND(
    (1 - (
      (SELECT VARIABLE_VALUE FROM performance_schema.global_status WHERE VARIABLE_NAME = 'Innodb_buffer_pool_reads') /
      (SELECT VARIABLE_VALUE FROM performance_schema.global_status WHERE VARIABLE_NAME = 'Innodb_buffer_pool_read_requests')
    )) * 100, 2
  ) AS buffer_hit_ratio_percent;`
      },
      {
        name: "Performance Schema - Connection Analysis",
        query: `SELECT 
  SUBSTRING_INDEX(host, ':', 1) AS client_host,
  COUNT(*) AS connection_count,
  SUM(IF(command = 'Sleep', 1, 0)) AS sleeping_connections,
  SUM(IF(command != 'Sleep', 1, 0)) AS active_connections
FROM information_schema.processlist 
GROUP BY SUBSTRING_INDEX(host, ':', 1)
ORDER BY connection_count DESC;`
      },
      {
        name: "Performance Schema - I/O Statistics",
        query: `SELECT 
  object_schema,
  object_name,
  count_read,
  count_write,
  sum_timer_read/1000000000 AS read_time_ms,
  sum_timer_write/1000000000 AS write_time_ms
FROM performance_schema.table_io_waits_summary_by_table
WHERE object_schema NOT IN ('mysql', 'performance_schema', 'information_schema', 'sys')
ORDER BY (count_read + count_write) DESC
LIMIT 10;`
      },
      {
        name: "Performance Schema - Memory Usage by Event",
        query: `SELECT 
  event_name,
  current_count_used,
  current_allocated,
  current_avg_alloc,
  current_max_alloc
FROM performance_schema.memory_summary_global_by_event_name
WHERE current_allocated > 0
ORDER BY current_allocated DESC
LIMIT 10;`
      },
      {
        name: "Performance Schema - Index Usage Statistics",
        query: `SELECT 
  object_schema,
  object_name,
  index_name,
  count_fetch,
  count_insert,
  count_update,
  count_delete
FROM performance_schema.table_io_waits_summary_by_index_usage
WHERE object_schema NOT IN ('mysql', 'performance_schema', 'information_schema', 'sys')
  AND count_fetch > 0
ORDER BY count_fetch DESC
LIMIT 10;`
      }
    ],
    postgresql: [
      {
        name: "List Tables",
        query: "SELECT schemaname, tablename FROM pg_tables WHERE schemaname NOT IN ('information_schema', 'pg_catalog');"
      },
      {
        name: "Database Size",
        query: "SELECT pg_size_pretty(pg_database_size(current_database())) AS database_size;"
      },
      {
        name: "Active Connections",
        query: "SELECT count(*) FROM pg_stat_activity WHERE state = 'active';"
      },
      {
        name: "Table Sizes",
        query: `SELECT 
  schemaname,
  tablename,
  pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size,
  pg_total_relation_size(schemaname||'.'||tablename) AS size_bytes
FROM pg_tables 
WHERE schemaname NOT IN ('information_schema', 'pg_catalog')
ORDER BY size_bytes DESC
LIMIT 10;`
      },
      {
        name: "Slow Queries",
        query: `SELECT 
  query,
  calls,
  total_time,
  mean_time,
  rows
FROM pg_stat_statements
ORDER BY total_time DESC
LIMIT 10;`
      }
    ]
  };

  const executeCurrentQuery = async () => {
    if (!currentQuery.trim()) return;
    await executeQuery();
  };

  const loadCommonQuery = (query) => {
    setCurrentQuery(query);
  };

  const formatQueryResult = (result) => {
    if (!result) return null;

    const columnDefs = result.columns.map(col => ({
      headerName: col,
      field: col,
      resizable: true,
      sortable: true,
      filter: true
    }));

    return {
      columnDefs,
      rowData: result.rows
    };
  };

  const gridData = formatQueryResult(queryResult);

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
                      <h6>Common Queries</h6>
                      {(commonQueries[connection.connection_type] || commonQueries.mysql).slice(0, 5).map((template, index) => (
                        <CButton
                          key={index}
                          variant="outline"
                          color="info"
                          size="sm"
                          className="me-2 mb-2 d-block w-100 text-start"
                          onClick={() => loadCommonQuery(template.query)}
                        >
                          {template.name}
                        </CButton>
                      ))}
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
                  <div className="ag-theme-alpine" style={{ height: "100%", width: "100%" }}>
                    <AgGridReact
                      ref={gridRef}
                      columnDefs={gridData.columnDefs}
                      rowData={gridData.rowData}
                      defaultColDef={{
                        resizable: true,
                        sortable: true,
                        filter: true
                      }}
                      pagination={true}
                      paginationPageSize={50}
                    />
                  </div>
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
              <CCardHeader>
                <strong>Query Templates - {connection.connection_type.toUpperCase()}</strong>
              </CCardHeader>
              <CCardBody style={{ height: "calc(100% - 60px)", overflowY: "auto" }}>
                <CRow>
                  {(commonQueries[connection.connection_type] || commonQueries.mysql).map((template, index) => (
                    <CCol lg={6} key={index} className="mb-3">
                      <CCard>
                        <CCardHeader className="d-flex justify-content-between align-items-center">
                          <strong>{template.name}</strong>
                          <CButton
                            size="sm"
                            color="primary"
                            onClick={() => {
                              loadCommonQuery(template.query);
                              setActiveTab("editor");
                            }}
                          >
                            Use Template
                          </CButton>
                        </CCardHeader>
                        <CCardBody>
                          <pre className="bg-light p-2 small" style={{ whiteSpace: "pre-wrap", fontSize: "0.8em" }}>
                            {template.query}
                          </pre>
                        </CCardBody>
                      </CCard>
                    </CCol>
                  ))}
                </CRow>
              </CCardBody>
            </CCard>
          </CTabPanel>
        </CTabContent>
      </CTabs>
    </div>
  );
};

export default QueryTool;
