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


import React, { useState, useEffect } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CSpinner,
  CAlert,
  CBadge,
  CCollapse,
  CAccordion,
  CAccordionItem,
  CAccordionHeader,
  CAccordionBody,
  CNav,
  CNavItem,
  CNavLink,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CFormInput,
  CInputGroup,
  CInputGroupText
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import api from "../../services/api";

const SchemaExplorer = ({ connection }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [schema, setSchema] = useState(null);
  const [activeTab, setActiveTab] = useState("tables");
  const [selectedTable, setSelectedTable] = useState(null);
  const [tableDetails, setTableDetails] = useState(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [expandedTables, setExpandedTables] = useState(new Set());

  useEffect(() => {
    if (connection) {
      fetchSchema();
    }
  }, [connection]);

  const fetchSchema = async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await api.get(`/api/databases/${connection.id}/schema`);
      setSchema(response.data);
    } catch (err) {
      setError("Failed to load schema: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const fetchTableDetails = async (tableName) => {
    try {
      setLoading(true);
      const response = await api.get(`/api/databases/${connection.id}/table/${tableName}/details`);
      setTableDetails(response.data);
      setSelectedTable(tableName);
    } catch (err) {
      setError("Failed to load table details: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const toggleTableExpansion = (tableName) => {
    const newExpanded = new Set(expandedTables);
    if (newExpanded.has(tableName)) {
      newExpanded.delete(tableName);
    } else {
      newExpanded.add(tableName);
    }
    setExpandedTables(newExpanded);
  };

  const getColumnTypeColor = (type) => {
    const lowerType = type.toLowerCase();
    if (lowerType.includes('int') || lowerType.includes('number') || lowerType.includes('decimal')) return 'info';
    if (lowerType.includes('char') || lowerType.includes('text') || lowerType.includes('string')) return 'primary';
    if (lowerType.includes('date') || lowerType.includes('time')) return 'warning';
    if (lowerType.includes('bool')) return 'success';
    return 'secondary';
  };

  const filteredTables = schema?.tables?.filter(table => 
    table.name.toLowerCase().includes(searchTerm.toLowerCase())
  ) || [];

  if (loading && !schema) {
    return (
      <div className="text-center p-5">
        <CSpinner color="primary" />
        <div className="mt-3">Loading database schema...</div>
      </div>
    );
  }

  if (error) {
    return <CAlert color="danger">{error}</CAlert>;
  }

  if (!schema) {
    return <CAlert color="info">No schema data available</CAlert>;
  }

  return (
    <div>
      <CRow>
        <CCol lg={4}>
          {/* Schema Tree */}
          <CCard className="h-100">
            <CCardHeader>
              <div className="d-flex justify-content-between align-items-center">
                <strong>📋 Database Schema</strong>
                <CButton size="sm" color="primary" variant="outline" onClick={fetchSchema}>
                  🔄 Refresh
                </CButton>
              </div>
            </CCardHeader>
            <CCardBody>
              {/* Search */}
              <CInputGroup className="mb-3">
                <CInputGroupText>🔍</CInputGroupText>
                <CFormInput
                  placeholder="Search tables..."
                  value={searchTerm}
                  onChange={(e) => setSearchTerm(e.target.value)}
                />
              </CInputGroup>

              {/* Schema Navigation */}
              <CNav variant="pills" orientation="vertical">
                <CNavItem>
                  <CNavLink
                    href="#"
                    active={activeTab === "tables"}
                    onClick={(e) => { e.preventDefault(); setActiveTab("tables"); }}
                  >
                    🗂️ Tables ({schema.tables?.length || 0})
                  </CNavLink>
                </CNavItem>
                {schema.views && schema.views.length > 0 && (
                  <CNavItem>
                    <CNavLink
                      href="#"
                      active={activeTab === "views"}
                      onClick={(e) => { e.preventDefault(); setActiveTab("views"); }}
                    >
                      👁️ Views ({schema.views.length})
                    </CNavLink>
                  </CNavItem>
                )}
                {schema.functions && schema.functions.length > 0 && (
                  <CNavItem>
                    <CNavLink
                      href="#"
                      active={activeTab === "functions"}
                      onClick={(e) => { e.preventDefault(); setActiveTab("functions"); }}
                    >
                      ⚙️ Functions ({schema.functions.length})
                    </CNavLink>
                  </CNavItem>
                )}
              </CNav>

              {/* Tables List */}
              {activeTab === "tables" && (
                <div className="mt-3">
                  {filteredTables.map((table, index) => (
                    <div key={index} className="mb-2">
                      <div 
                        className="d-flex align-items-center p-2 border rounded cursor-pointer"
                        style={{ cursor: 'pointer' }}
                        onClick={() => toggleTableExpansion(table.name)}
                      >
                        <span className="me-2">
                          {expandedTables.has(table.name) ? '📂' : '📁'}
                        </span>
                        <span className="flex-grow-1">{table.name}</span>
                        <CBadge color="info" size="sm">
                          {table.columns?.length || 0} cols
                        </CBadge>
                      </div>
                      
                      <CCollapse visible={expandedTables.has(table.name)}>
                        <div className="ms-4 mt-2">
                          {table.columns?.slice(0, 5).map((column, idx) => (
                            <div key={idx} className="d-flex justify-content-between align-items-center py-1">
                              <span className="small">
                                {column.nullable === false && <strong>*</strong>}
                                {column.name}
                              </span>
                              <CBadge color={getColumnTypeColor(column.type)} size="sm">
                                {column.type}
                              </CBadge>
                            </div>
                          ))}
                          {table.columns?.length > 5 && (
                            <div className="text-muted small">
                              ... and {table.columns.length - 5} more columns
                            </div>
                          )}
                          <CButton
                            size="sm"
                            color="primary"
                            variant="outline"
                            className="mt-2"
                            onClick={(e) => {
                              e.stopPropagation();
                              fetchTableDetails(table.name);
                            }}
                          >
                            View Details
                          </CButton>
                        </div>
                      </CCollapse>
                    </div>
                  ))}
                </div>
              )}

              {/* Views List */}
              {activeTab === "views" && schema.views && (
                <div className="mt-3">
                  {schema.views.map((view, index) => (
                    <div key={index} className="p-2 border rounded mb-2">
                      <div className="d-flex justify-content-between align-items-center">
                        <span>👁️ {view.name}</span>
                        <CBadge color="secondary" size="sm">VIEW</CBadge>
                      </div>
                    </div>
                  ))}
                </div>
              )}

              {/* Functions List */}
              {activeTab === "functions" && schema.functions && (
                <div className="mt-3">
                  {schema.functions.map((func, index) => (
                    <div key={index} className="p-2 border rounded mb-2">
                      <div className="d-flex justify-content-between align-items-center">
                        <span>⚙️ {func.name}</span>
                        <CBadge color="success" size="sm">FUNCTION</CBadge>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </CCardBody>
          </CCard>
        </CCol>

        <CCol lg={8}>
          {/* Table Details */}
          {selectedTable && tableDetails ? (
            <CCard>
              <CCardHeader>
                <div className="d-flex justify-content-between align-items-center">
                  <div>
                    <strong>📋 {selectedTable}</strong>
                    <div className="text-muted small">Table Details</div>
                  </div>
                  <div className="d-flex gap-2">
                    <CBadge color="info">
                      {tableDetails.columns?.length || 0} columns
                    </CBadge>
                    <CBadge color="success">
                      {tableDetails.row_count?.toLocaleString() || 'N/A'} rows
                    </CBadge>
                  </div>
                </div>
              </CCardHeader>
              <CCardBody>
                <CAccordion>
                  {/* Columns */}
                  <CAccordionItem>
                    <CAccordionHeader>
                      📊 Columns ({tableDetails.columns?.length || 0})
                    </CAccordionHeader>
                    <CAccordionBody>
                      <CTable striped hover responsive>
                        <CTableHead>
                          <CTableRow>
                            <CTableHeaderCell>Column</CTableHeaderCell>
                            <CTableHeaderCell>Type</CTableHeaderCell>
                            <CTableHeaderCell>Nullable</CTableHeaderCell>
                            <CTableHeaderCell>Default</CTableHeaderCell>
                            <CTableHeaderCell>Key</CTableHeaderCell>
                          </CTableRow>
                        </CTableHead>
                        <CTableBody>
                          {tableDetails.columns?.map((column, index) => (
                            <CTableRow key={index}>
                              <CTableDataCell>
                                <strong>{column.name}</strong>
                              </CTableDataCell>
                              <CTableDataCell>
                                <CBadge color={getColumnTypeColor(column.type)}>
                                  {column.type}
                                </CBadge>
                              </CTableDataCell>
                              <CTableDataCell>
                                {column.nullable ? 
                                  <CBadge color="warning">YES</CBadge> : 
                                  <CBadge color="danger">NO</CBadge>
                                }
                              </CTableDataCell>
                              <CTableDataCell>
                                <code>{column.default || 'NULL'}</code>
                              </CTableDataCell>
                              <CTableDataCell>
                                {column.is_primary && <CBadge color="danger" className="me-1">PK</CBadge>}
                                {column.is_foreign_key && <CBadge color="info" className="me-1">FK</CBadge>}
                                {column.is_unique && <CBadge color="success">UQ</CBadge>}
                              </CTableDataCell>
                            </CTableRow>
                          ))}
                        </CTableBody>
                      </CTable>
                    </CAccordionBody>
                  </CAccordionItem>

                  {/* Indexes */}
                  {tableDetails.indexes && tableDetails.indexes.length > 0 && (
                    <CAccordionItem>
                      <CAccordionHeader>
                        🗂️ Indexes ({tableDetails.indexes.length})
                      </CAccordionHeader>
                      <CAccordionBody>
                        <CTable striped hover responsive>
                          <CTableHead>
                            <CTableRow>
                              <CTableHeaderCell>Index Name</CTableHeaderCell>
                              <CTableHeaderCell>Columns</CTableHeaderCell>
                              <CTableHeaderCell>Type</CTableHeaderCell>
                              <CTableHeaderCell>Unique</CTableHeaderCell>
                            </CTableRow>
                          </CTableHead>
                          <CTableBody>
                            {tableDetails.indexes.map((index, idx) => (
                              <CTableRow key={idx}>
                                <CTableDataCell>
                                  <strong>{index.name}</strong>
                                </CTableDataCell>
                                <CTableDataCell>
                                  {index.columns?.join(', ') || 'N/A'}
                                </CTableDataCell>
                                <CTableDataCell>
                                  <CBadge color={index.is_primary ? 'danger' : 'info'}>
                                    {index.is_primary ? 'PRIMARY' : index.type || 'INDEX'}
                                  </CBadge>
                                </CTableDataCell>
                                <CTableDataCell>
                                  {index.is_unique ? 
                                    <CBadge color="success">YES</CBadge> : 
                                    <CBadge color="secondary">NO</CBadge>
                                  }
                                </CTableDataCell>
                              </CTableRow>
                            ))}
                          </CTableBody>
                        </CTable>
                      </CAccordionBody>
                    </CAccordionItem>
                  )}

                  {/* Foreign Keys */}
                  {tableDetails.foreign_keys && tableDetails.foreign_keys.length > 0 && (
                    <CAccordionItem>
                      <CAccordionHeader>
                        🔗 Foreign Keys ({tableDetails.foreign_keys.length})
                      </CAccordionHeader>
                      <CAccordionBody>
                        <CTable striped hover responsive>
                          <CTableHead>
                            <CTableRow>
                              <CTableHeaderCell>Constraint</CTableHeaderCell>
                              <CTableHeaderCell>Column</CTableHeaderCell>
                              <CTableHeaderCell>Referenced Table</CTableHeaderCell>
                              <CTableHeaderCell>Referenced Column</CTableHeaderCell>
                            </CTableRow>
                          </CTableHead>
                          <CTableBody>
                            {tableDetails.foreign_keys.map((fk, idx) => (
                              <CTableRow key={idx}>
                                <CTableDataCell>
                                  <strong>{fk.constraint_name}</strong>
                                </CTableDataCell>
                                <CTableDataCell>{fk.column}</CTableDataCell>
                                <CTableDataCell>
                                  <CBadge color="info">{fk.referenced_table}</CBadge>
                                </CTableDataCell>
                                <CTableDataCell>{fk.referenced_column}</CTableDataCell>
                              </CTableRow>
                            ))}
                          </CTableBody>
                        </CTable>
                      </CAccordionBody>
                    </CAccordionItem>
                  )}

                  {/* Table Statistics */}
                  {tableDetails.statistics && (
                    <CAccordionItem>
                      <CAccordionHeader>
                        📈 Statistics
                      </CAccordionHeader>
                      <CAccordionBody>
                        <CRow>
                          <CCol md={3}>
                            <div className="text-center p-3 border rounded">
                              <div className="fs-4 fw-semibold text-primary">
                                {tableDetails.statistics.row_count?.toLocaleString() || 'N/A'}
                              </div>
                              <div className="text-muted small">Total Rows</div>
                            </div>
                          </CCol>
                          <CCol md={3}>
                            <div className="text-center p-3 border rounded">
                              <div className="fs-4 fw-semibold text-info">
                                {formatBytes(tableDetails.statistics.table_size_bytes || 0)}
                              </div>
                              <div className="text-muted small">Table Size</div>
                            </div>
                          </CCol>
                          <CCol md={3}>
                            <div className="text-center p-3 border rounded">
                              <div className="fs-4 fw-semibold text-warning">
                                {formatBytes(tableDetails.statistics.index_size_bytes || 0)}
                              </div>
                              <div className="text-muted small">Index Size</div>
                            </div>
                          </CCol>
                          <CCol md={3}>
                            <div className="text-center p-3 border rounded">
                              <div className="fs-4 fw-semibold text-success">
                                {tableDetails.statistics.avg_row_length || 'N/A'}
                              </div>
                              <div className="text-muted small">Avg Row Length</div>
                            </div>
                          </CCol>
                        </CRow>
                      </CAccordionBody>
                    </CAccordionItem>
                  )}
                </CAccordion>
              </CCardBody>
            </CCard>
          ) : (
            <CCard>
              <CCardBody className="text-center text-muted">
                <div className="fs-1 mb-3">📋</div>
                <h5>Schema Explorer</h5>
                <p>Select a table from the schema tree to view detailed information including columns, indexes, foreign keys, and statistics.</p>
                {loading && <CSpinner color="primary" className="mt-3" />}
              </CCardBody>
            </CCard>
          )}
        </CCol>
      </CRow>
    </div>
  );
};

// Utility function
const formatBytes = (bytes) => {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

export default SchemaExplorer;
