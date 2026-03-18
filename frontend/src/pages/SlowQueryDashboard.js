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
import { useNavigate } from "react-router-dom";
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
  CTabPane
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import { CChart } from "@coreui/react-chartjs";
import PageHeader from "../components/layout/PageHeader";
import {
  getSlowQueryEvents,
  getSlowQueryStatistics,
  analyzeSlowQueries,
  getAuroraClusters,
  getTopOffendingTables
} from "../services/api";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";

const SlowQueryDashboard = () => {
  const navigate = useNavigate();
  
  // State management
  const [clusters, setClusters] = useState([]);
  const [selectedCluster, setSelectedCluster] = useState(null);
  const [slowQueries, setSlowQueries] = useState([]);
  const [statistics, setStatistics] = useState(null);
  const [topTables, setTopTables] = useState([]);
  const [loading, setLoading] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Filter state
  const [filters, setFilters] = useState({
    hours: 24,
    min_execution_time: 1,
    limit: 100,
  });

  // Load clusters on component mount
  useEffect(() => {
    fetchClusters();
  }, []);

  // Load data when cluster or hours change
  useEffect(() => {
    if (selectedCluster) {
      fetchAllData();
    }
  }, [selectedCluster, filters.hours]);

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

  const fetchAllData = async () => {
    if (!selectedCluster) return;
    setLoading(true);
    try {
      const [queries, stats, tables] = await Promise.all([
        getSlowQueryEvents({ cluster_id: selectedCluster.id, ...filters }),
        getSlowQueryStatistics(selectedCluster.id, filters),
        getTopOffendingTables(selectedCluster.id, filters.hours)
      ]);
      setSlowQueries(queries || []);
      setStatistics(stats);
      setTopTables(tables || []);
    } catch (err) {
      setError("Failed to fetch dashboard data: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleExportCSV = () => {
    if (slowQueries.length === 0) return;
    
    const headers = ["Timestamp", "Execution Time", "Lock Time", "Rows Examined", "Rows Sent", "Database", "User", "SQL"];
    const csvRows = [
      headers.join(","),
      ...slowQueries.map(q => [
        new Date(q.start_time).toISOString(),
        q.execution_time,
        q.lock_time,
        q.rows_examined,
        q.rows_sent,
        q.db,
        q.user_host,
        `"${q.sql_text.replace(/"/g, '""')}"`
      ].join(","))
    ];
    
    const blob = new Blob([csvRows.join("\n")], { type: 'text/csv' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.setAttribute('hidden', '');
    a.setAttribute('href', url);
    a.setAttribute('download', `slow_queries_${selectedCluster.cluster_identifier}_${new Date().toISOString()}.csv`);
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  };

  const formatDuration = (seconds) => {
    if (seconds < 1) return `${(seconds * 1000).toFixed(2)}ms`;
    if (seconds < 60) return `${seconds.toFixed(2)}s`;
    return `${Math.floor(seconds / 60)}m ${(seconds % 60).toFixed(2)}s`;
  };

  return (
    <div className="slow-query-dashboard">
      <PageHeader
        title="Slow Query Dashboard"
        breadcrumbs={[
          { label: "Performance Analysis", link: "/performance-analysis" },
          { label: "Slow Query Dashboard" }
        ]}
      />

      {error && <CAlert color="danger" dismissible onClose={() => setError(null)}>{error}</CAlert>}
      
      <CCard className="mb-4 shadow-sm border-0">
        <CCardBody>
          <CRow className="align-items-end">
            <CCol md={4}>
              <CFormLabel className="fw-bold">Aurora Cluster</CFormLabel>
              <CFormSelect
                value={selectedCluster?.id || ""}
                onChange={(e) => setSelectedCluster(clusters.find(c => c.id === parseInt(e.target.value)))}
              >
                {clusters.map(c => <option key={c.id} value={c.id}>{c.cluster_identifier}</option>)}
              </CFormSelect>
            </CCol>
            <CCol md={2}>
              <CFormLabel className="fw-bold">Time Range</CFormLabel>
              <CFormSelect
                value={filters.hours}
                onChange={(e) => setFilters(f => ({ ...f, hours: parseInt(e.target.value) }))}
              >
                <option value={1}>Last Hour</option>
                <option value={24}>Last 24 Hours</option>
                <option value={168}>Last 7 Days</option>
              </CFormSelect>
            </CCol>
            <CCol md={6} className="text-end">
              <CButton color="success" onClick={handleExportCSV} disabled={slowQueries.length === 0} className="me-2 text-white">
                Export CSV
              </CButton>
              <CButton color="primary" onClick={fetchAllData} disabled={loading}>
                {loading ? <CSpinner size="sm" /> : "Refresh"}
              </CButton>
            </CCol>
          </CRow>
        </CCardBody>
      </CCard>

      <CRow className="mb-4">
        <CCol md={3}>
          <CCard className="h-100 shadow-sm border-0 text-white bg-primary">
            <CCardBody className="d-flex flex-column justify-content-center text-center">
              <h2 className="mb-0">{statistics?.total_queries || 0}</h2>
              <p className="mb-0 opacity-75">Total Slow Queries</p>
            </CCardBody>
          </CCard>
        </CCol>
        <CCol md={3}>
          <CCard className="h-100 shadow-sm border-0 text-white bg-info">
            <CCardBody className="d-flex flex-column justify-content-center text-center">
              <h2 className="mb-0">{formatDuration(statistics?.avg_execution_time || 0)}</h2>
              <p className="mb-0 opacity-75">Avg Latency</p>
            </CCardBody>
          </CCard>
        </CCol>
        <CCol md={3}>
          <CCard className="h-100 shadow-sm border-0 text-white bg-warning">
            <CCardBody className="d-flex flex-column justify-content-center text-center">
              <h2 className="mb-0">{statistics?.unique_queries || 0}</h2>
              <p className="mb-0 opacity-75">Fingerprints</p>
            </CCardBody>
          </CCard>
        </CCol>
        <CCol md={3}>
          <CCard className="h-100 shadow-sm border-0 text-white bg-danger">
            <CCardBody className="d-flex flex-column justify-content-center text-center">
              <h2 className="mb-0">{formatDuration(statistics?.max_execution_time || 0)}</h2>
              <p className="mb-0 opacity-75">Peak Latency</p>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      <CRow className="mb-4">
        <CCol md={8}>
          <CCard className="h-100 shadow-sm border-0">
            <CCardHeader className="bg-white fw-bold">Latency Distribution</CCardHeader>
            <CCardBody>
              <div style={{ height: '300px' }}>
                <CChart
                  type="bar"
                  data={{
                    labels: statistics?.execution_time_distribution?.map(d => d.range) || [],
                    datasets: [{
                      label: 'Count',
                      data: statistics?.execution_time_distribution?.map(d => d.count) || [],
                      backgroundColor: '#36A2EB'
                    }]
                  }}
                  options={{ responsive: true, maintainAspectRatio: false }}
                />
              </div>
            </CCardBody>
          </CCard>
        </CCol>
        <CCol md={4}>
          <CCard className="h-100 shadow-sm border-0">
            <CCardHeader className="bg-white fw-bold">Top Offending Tables</CCardHeader>
            <CCardBody>
              <CTable hover responsive align="middle">
                <CTableHead>
                  <CTableRow>
                    <CTableHeaderCell>Table</CTableHeaderCell>
                    <CTableHeaderCell className="text-end">Total Time</CTableHeaderCell>
                  </CTableRow>
                </CTableHead>
                <CTableBody>
                  {topTables.map((t, idx) => (
                    <CTableRow key={idx}>
                      <CTableDataCell className="fw-semibold text-primary">{t.table_name}</CTableDataCell>
                      <CTableDataCell className="text-end">{formatDuration(t.total_query_time)}</CTableDataCell>
                    </CTableRow>
                  ))}
                  {topTables.length === 0 && (
                    <CTableRow><CTableDataCell colSpan="2" className="text-center text-muted">No data</CTableDataCell></CTableRow>
                  )}
                </CTableBody>
              </CTable>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      {/* Slow Query Events Grid */}
      <CCard className="shadow-sm border-0">
        <CCardHeader className="bg-white fw-bold d-flex justify-content-between align-items-center">
          <span>Recent Slow Query Events</span>
          <CBadge color="info">{slowQueries.length} Total</CBadge>
        </CCardHeader>
        <CCardBody>
          <div className="ag-theme-alpine" style={{ height: '500px', width: '100%' }}>
            <AgGridReact
              rowData={slowQueries}
              columnDefs={[
                { headerName: "Timestamp", field: "start_time", valueFormatter: p => new Date(p.value).toLocaleString(), width: 180 },
                { headerName: "Latency", field: "execution_time", valueFormatter: p => formatDuration(p.value), width: 120 },
                { headerName: "Database", field: "db", width: 120 },
                { headerName: "SQL", field: "sql_text", flex: 1, minWidth: 300 },
                { 
                  headerName: "Actions", 
                  field: "fingerprint_id", 
                  width: 130, 
                  cellRenderer: (params) => (
                    <CButton 
                      size="sm" 
                      color="primary" 
                      variant="outline" 
                      onClick={() => navigate(`/query-fingerprints/${params.value}`)}
                      disabled={!params.value}
                    >
                      View Pattern
                    </CButton>
                  )
                }
              ]}
              pagination={true}
              paginationPageSize={10}
              defaultColDef={{ resizable: true, sortable: true, filter: true }}
            />
          </div>
        </CCardBody>
      </CCard>
    </div>
  );
};

export default SlowQueryDashboard;
