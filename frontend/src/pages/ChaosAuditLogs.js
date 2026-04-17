import React, { useState, useEffect } from 'react';
import {
  CCard,
  CCardBody,
  CCardHeader,
  CTable,
  CTableBody,
  CTableDataCell,
  CTableHead,
  CTableHeaderCell,
  CTableRow,
  CRow,
  CCol,
  CButton,
  CForm,
  CFormInput,
  CFormSelect,
  CBadge,
  CPagination,
  CPaginationItem,
  CAlert,
  CSpinner,
} from '@coreui/react';
import CIcon from '@coreui/icons-react';
import { cilDownload, cilExternalLink } from '@coreui/icons';
import auditService from '../services/auditService';

const ChaosAuditLogs = () => {
  const [logs, setLogs] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(50);
  const [totalPages, setTotalPages] = useState(1);

  const [filters, setFilters] = useState({
    action: '',
    user_id: '',
    resource_id: '',
    triggered_by: '',
    start_date: '',
    end_date: '',
  });

  const actionColors = {
    template_created: 'success',
    template_updated: 'info',
    template_deleted: 'danger',
    experiment_created: 'success',
    experiment_updated: 'info',
    experiment_deleted: 'danger',
    run_started: 'warning',
    run_completed: 'success',
    run_failed: 'danger',
    run_stopped: 'dark',
    run_timed_out: 'danger',
    rollback_started: 'warning',
    rollback_completed: 'success',
    rollback_failed: 'danger',
  };

  const fetchLogs = async (pageNum = 1) => {
    setLoading(true);
    setError(null);
    try {
      const result = await auditService.listAuditLogs({
        ...filters,
        page: pageNum,
        page_size: pageSize,
      });
      setLogs(result.logs || []);
      setPage(pageNum);
      setTotalPages(result.total_pages || 1);
    } catch (err) {
      setError(err.response?.data?.message || 'Failed to fetch audit logs');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchLogs(1);
  }, [filters, pageSize]);

  const handleFilterChange = (field, value) => {
    setFilters(prev => ({
      ...prev,
      [field]: value,
    }));
    setPage(1);
  };

  const handleExport = () => {
    auditService.exportAuditLogs(filters);
  };

  const formatDate = (dateString) => {
    return new Date(dateString).toLocaleString();
  };

  const getPaginationItems = () => {
    const items = [];
    const maxPagesToShow = 5;
    let startPage = Math.max(1, page - Math.floor(maxPagesToShow / 2));
    let endPage = Math.min(totalPages, startPage + maxPagesToShow - 1);

    if (endPage - startPage < maxPagesToShow - 1) {
      startPage = Math.max(1, endPage - maxPagesToShow + 1);
    }

    if (startPage > 1) {
      items.push(
        <CPaginationItem key="first" onClick={() => fetchLogs(1)}>
          First
        </CPaginationItem>
      );
    }

    for (let i = startPage; i <= endPage; i++) {
      items.push(
        <CPaginationItem
          key={i}
          active={i === page}
          onClick={() => fetchLogs(i)}
        >
          {i}
        </CPaginationItem>
      );
    }

    if (endPage < totalPages) {
      items.push(
        <CPaginationItem key="last" onClick={() => fetchLogs(totalPages)}>
          Last
        </CPaginationItem>
      );
    }

    return items;
  };

  return (
    <CRow className="mb-3">
      <CCol xs={12}>
        <CCard>
          <CCardHeader>
            <div className="d-flex justify-content-between align-items-center">
              <span>Audit Logs</span>
              <div className="gap-2 d-flex">
                <CButton color="primary" size="sm" onClick={handleExport} disabled={logs.length === 0}>
                  <CIcon icon={cilDownload} className="me-2" />
                  Export CSV
                </CButton>
              </div>
            </div>
          </CCardHeader>
          <CCardBody>
            {/* Filters */}
            <CForm className="mb-3 p-3 bg-light rounded">
              <h6 className="mb-3">Filters</h6>
              <CRow>
                <CCol md={6} lg={4} className="mb-3">
                  <label className="form-label small">Action</label>
                  <CFormSelect
                    size="sm"
                    value={filters.action}
                    onChange={(e) => handleFilterChange('action', e.target.value)}
                  >
                    <option value="">All Actions</option>
                    <option value="template_created">Template Created</option>
                    <option value="template_updated">Template Updated</option>
                    <option value="template_deleted">Template Deleted</option>
                    <option value="experiment_created">Experiment Created</option>
                    <option value="experiment_updated">Experiment Updated</option>
                    <option value="experiment_deleted">Experiment Deleted</option>
                    <option value="run_started">Run Started</option>
                    <option value="run_completed">Run Completed</option>
                    <option value="run_failed">Run Failed</option>
                    <option value="run_stopped">Run Stopped</option>
                    <option value="rollback_started">Rollback Started</option>
                    <option value="rollback_completed">Rollback Completed</option>
                    <option value="rollback_failed">Rollback Failed</option>
                  </CFormSelect>
                </CCol>

                <CCol md={6} lg={4} className="mb-3">
                  <label className="form-label small">User ID</label>
                  <CFormInput
                    size="sm"
                    type="text"
                    placeholder="Filter by user..."
                    value={filters.user_id}
                    onChange={(e) => handleFilterChange('user_id', e.target.value)}
                  />
                </CCol>

                <CCol md={6} lg={4} className="mb-3">
                  <label className="form-label small">Resource ID</label>
                  <CFormInput
                    size="sm"
                    type="text"
                    placeholder="Filter by resource..."
                    value={filters.resource_id}
                    onChange={(e) => handleFilterChange('resource_id', e.target.value)}
                  />
                </CCol>

                <CCol md={6} lg={4} className="mb-3">
                  <label className="form-label small">Triggered By</label>
                  <CFormSelect
                    size="sm"
                    value={filters.triggered_by}
                    onChange={(e) => handleFilterChange('triggered_by', e.target.value)}
                  >
                    <option value="">All Sources</option>
                    <option value="ui_user">UI User</option>
                    <option value="scheduler">Scheduler</option>
                    <option value="api">API</option>
                    <option value="cli">CLI</option>
                    <option value="system">System</option>
                  </CFormSelect>
                </CCol>

                <CCol md={6} lg={4} className="mb-3">
                  <label className="form-label small">Start Date</label>
                  <CFormInput
                    size="sm"
                    type="datetime-local"
                    value={filters.start_date}
                    onChange={(e) => handleFilterChange('start_date', e.target.value)}
                  />
                </CCol>

                <CCol md={6} lg={4} className="mb-3">
                  <label className="form-label small">End Date</label>
                  <CFormInput
                    size="sm"
                    type="datetime-local"
                    value={filters.end_date}
                    onChange={(e) => handleFilterChange('end_date', e.target.value)}
                  />
                </CCol>
              </CRow>
            </CForm>

            {/* Error Alert */}
            {error && <CAlert color="danger" className="mb-3" dismissible onClose={() => setError(null)}>{error}</CAlert>}

            {/* Loading State */}
            {loading ? (
              <div className="text-center py-4">
                <CSpinner color="primary" />
              </div>
            ) : logs.length === 0 ? (
              <CAlert color="info">No audit logs found matching the filters</CAlert>
            ) : (
              <>
                {/* Table */}
                <div className="table-responsive">
                  <CTable bordered hover>
                    <CTableHead>
                      <CTableHeaderCell>Timestamp</CTableHeaderCell>
                      <CTableHeaderCell>Action</CTableHeaderCell>
                      <CTableHeaderCell>User</CTableHeaderCell>
                      <CTableHeaderCell>Triggered By</CTableHeaderCell>
                      <CTableHeaderCell>Resource ID</CTableHeaderCell>
                      <CTableHeaderCell>Status Change</CTableHeaderCell>
                      <CTableHeaderCell>IP Address</CTableHeaderCell>
                    </CTableHeaderCell>
                  </CTableHead>
                  <CTableBody>
                    {logs.map((log) => (
                      <CTableRow key={log.id}>
                        <CTableDataCell>
                          <small>{formatDate(log.created_at)}</small>
                        </CTableDataCell>
                        <CTableDataCell>
                          <CBadge color={actionColors[log.action] || 'secondary'}>
                            {log.action}
                          </CBadge>
                        </CTableDataCell>
                        <CTableDataCell>
                          <small>{log.user_id || '-'}</small>
                        </CTableDataCell>
                        <CTableDataCell>
                          <small>{log.triggered_by || '-'}</small>
                        </CTableDataCell>
                        <CTableDataCell>
                          <small>{log.resource_id || '-'}</small>
                        </CTableDataCell>
                        <CTableDataCell>
                          <small>
                            {log.status_before ? `${log.status_before} → ${log.status_after}` : '-'}
                          </small>
                        </CTableDataCell>
                        <CTableDataCell>
                          <small>{log.ip_address || '-'}</small>
                        </CTableDataCell>
                      </CTableRow>
                    ))}
                  </CTableBody>
                </div>

                {/* Pagination */}
                <CRow className="mt-3 align-items-center">
                  <CCol md={6}>
                    <small>
                      Showing {(page - 1) * pageSize + 1} to {Math.min(page * pageSize, logs.length)} of ~{totalPages * pageSize} logs
                    </small>
                  </CCol>
                  <CCol md={6} className="text-end">
                    <CPagination className="justify-content-end">
                      <CPaginationItem
                        disabled={page === 1}
                        onClick={() => fetchLogs(page - 1)}
                      >
                        Previous
                      </CPaginationItem>
                      {getPaginationItems()}
                      <CPaginationItem
                        disabled={page === totalPages}
                        onClick={() => fetchLogs(page + 1)}
                      >
                        Next
                      </CPaginationItem>
                    </CPagination>
                  </CCol>
                </CRow>
              </>
            )}
          </CCardBody>
        </CCard>
      </CCol>
    </CRow>
  );
};

export default ChaosAuditLogs;
