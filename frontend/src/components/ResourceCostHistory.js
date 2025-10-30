import React, { useState, useEffect } from 'react';
import {
  CContainer,
  CRow,
  CCol,
  CCard,
  CCardBody,
  CCardHeader,
  CCardTitle,
  CButton,
  CForm,
  CFormInput,
  CFormSelect,
  CAlert,
  CSpinner,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CBadge,
  CModal,
  CModalHeader,
  CModalTitle,
  CModalBody,
  CModalFooter
} from '@coreui/react';
import { CChartLine } from '@coreui/react-chartjs';
import { apiCall } from '../services/api';

const ResourceCostHistory = ({ resourceId, accountId, onClose }) => {
  const [loading, setLoading] = useState(false);
  const [costHistory, setCostHistory] = useState(null);
  const [error, setError] = useState(null);

  // Form state
  const [formData, setFormData] = useState({
    daysBack: 30,
    granularity: 'daily',
    format: 'json'
  });

  const fetchCostHistory = async () => {
    setLoading(true);
    setError(null);

    try {
      const params = new URLSearchParams({
        resource_id: resourceId,
        days_back: formData.daysBack,
        granularity: formData.granularity,
        format: formData.format
      });

      if (accountId) {
        params.append('account_id', accountId);
      }

      const response = await apiCall(`/api/cost-analytics/resource-history?${params}`);

      if (formData.format === 'csv') {
        // Handle CSV download
        const blob = new Blob([response], { type: 'text/csv' });
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${resourceId}_cost_history.csv`;
        document.body.appendChild(a);
        a.click();
        window.URL.revokeObjectURL(url);
        document.body.removeChild(a);
      } else {
        setCostHistory(response);
      }
    } catch (err) {
      setError(err.message || 'Failed to fetch cost history');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchCostHistory();
  }, [resourceId, accountId]);

  const handleFormSubmit = (e) => {
    e.preventDefault();
    fetchCostHistory();
  };

  const exportCSV = async () => {
    setFormData(prev => ({ ...prev, format: 'csv' }));
    // Trigger fetch with CSV format
    setTimeout(() => {
      fetchCostHistory();
      setFormData(prev => ({ ...prev, format: 'json' }));
    }, 100);
  };

  return (
    <CContainer fluid>
      <CRow>
        <CCol>
          <CCard>
            <CCardHeader>
              <CCardTitle>Resource Cost History - {resourceId}</CCardTitle>
              <CButton
                color="secondary"
                size="sm"
                onClick={onClose}
                className="float-end"
              >
                Close
              </CButton>
            </CCardHeader>
            <CCardBody>
              {/* Form for parameters */}
              <CForm onSubmit={handleFormSubmit} className="mb-4">
                <CRow>
                  <CCol md={3}>
                    <CFormSelect
                      label="Days Back"
                      value={formData.daysBack}
                      onChange={(e) => setFormData(prev => ({ ...prev, daysBack: parseInt(e.target.value) }))}
                    >
                      <option value={7}>7 days</option>
                      <option value={30}>30 days</option>
                      <option value={90}>90 days</option>
                      <option value={180}>180 days</option>
                      <option value={365}>365 days</option>
                    </CFormSelect>
                  </CCol>
                  <CCol md={3}>
                    <CFormSelect
                      label="Granularity"
                      value={formData.granularity}
                      onChange={(e) => setFormData(prev => ({ ...prev, granularity: e.target.value }))}
                    >
                      <option value="daily">Daily</option>
                      <option value="weekly">Weekly</option>
                    </CFormSelect>
                  </CCol>
                  <CCol md={3}>
                    <CButton type="submit" color="primary" disabled={loading}>
                      {loading ? <CSpinner size="sm" /> : 'Update'}
                    </CButton>
                  </CCol>
                  <CCol md={3}>
                    <CButton color="success" onClick={exportCSV} disabled={loading}>
                      Export CSV
                    </CButton>
                  </CCol>
                </CRow>
              </CForm>

              {error && (
                <CAlert color="danger" dismissible onClose={() => setError(null)}>
                  {error}
                </CAlert>
              )}

              {loading && (
                <div className="text-center">
                  <CSpinner />
                  <p>Loading cost history...</p>
                </div>
              )}

              {costHistory && !loading && (
                <>
                  {/* Summary Cards */}
                  <CRow className="mb-4">
                    <CCol md={3}>
                      <CCard className="text-center">
                        <CCardBody>
                          <h4>${costHistory.summary?.total_cost?.toFixed(2) || '0.00'}</h4>
                          <small className="text-muted">Total Cost</small>
                        </CCardBody>
                      </CCard>
                    </CCol>
                    <CCol md={3}>
                      <CCard className="text-center">
                        <CCardBody>
                          <h4>${costHistory.summary?.average_cost_per_period?.toFixed(2) || '0.00'}</h4>
                          <small className="text-muted">Avg per Period</small>
                        </CCardBody>
                      </CCard>
                    </CCol>
                    <CCol md={3}>
                      <CCard className="text-center">
                        <CCardBody>
                          <h4>${costHistory.summary?.max_cost?.toFixed(2) || '0.00'}</h4>
                          <small className="text-muted">Peak Cost</small>
                        </CCardBody>
                      </CCard>
                    </CCol>
                    <CCol md={3}>
                      <CCard className="text-center">
                        <CCardBody>
                          <h4>{costHistory.summary?.total_periods || 0}</h4>
                          <small className="text-muted">Data Points</small>
                        </CCardBody>
                      </CCard>
                    </CCol>
                  </CRow>

                  {/* Chart */}
                  <CRow className="mb-4">
                    <CCol>
                      <CCard>
                        <CCardHeader>
                          <CCardTitle>Cost Trend</CCardTitle>
                        </CCardHeader>
                        <CCardBody>
                          <CChartLine
                            data={{
                              labels: costHistory.chart_ready?.labels || [],
                              datasets: costHistory.chart_ready?.datasets || []
                            }}
                            options={{
                              responsive: true,
                              interaction: {
                                mode: 'index',
                                intersect: false,
                              },
                              scales: {
                                x: {
                                  display: true,
                                  title: {
                                    display: true,
                                    text: costHistory.granularity === 'weekly' ? 'Week' : 'Date'
                                  }
                                },
                                y: {
                                  type: 'linear',
                                  display: true,
                                  position: 'left',
                                  title: {
                                    display: true,
                                    text: 'Daily Cost ($)'
                                  },
                                  ticks: {
                                    callback: (value) => `$${value}`
                                  }
                                },
                                cumulative: {
                                  type: 'linear',
                                  display: true,
                                  position: 'right',
                                  title: {
                                    display: true,
                                    text: 'Cumulative Cost ($)'
                                  },
                                  ticks: {
                                    callback: (value) => `$${value}`
                                  },
                                  grid: {
                                    drawOnChartArea: false,
                                  },
                                },
                              }
                            }}
                          />
                        </CCardBody>
                      </CCard>
                    </CCol>
                  </CRow>

                  {/* Data Table */}
                  <CRow>
                    <CCol>
                      <CCard>
                        <CCardHeader>
                          <CCardTitle>Detailed Cost Data</CCardTitle>
                        </CCardHeader>
                        <CCardBody>
                          <CTable striped hover responsive>
                            <CTableHead>
                              <CTableRow>
                                <CTableHeaderCell>Period</CTableHeaderCell>
                                <CTableHeaderCell>Total Cost</CTableHeaderCell>
                                <CTableHeaderCell>Avg Daily Cost</CTableHeaderCell>
                                <CTableHeaderCell>Cumulative Cost</CTableHeaderCell>
                                <CTableHeaderCell>Data Points</CTableHeaderCell>
                              </CTableRow>
                            </CTableHead>
                            <CTableBody>
                              {costHistory.data?.map((item, index) => (
                                <CTableRow key={index}>
                                  <CTableDataCell>{item.formatted_period || item.period}</CTableDataCell>
                                  <CTableDataCell>${item.total_cost?.toFixed(2) || '0.00'}</CTableDataCell>
                                  <CTableDataCell>${item.avg_daily_cost?.toFixed(2) || '0.00'}</CTableDataCell>
                                  <CTableDataCell>${item.cumulative_cost?.toFixed(2) || '0.00'}</CTableDataCell>
                                  <CTableDataCell>{item.data_points || 0}</CTableDataCell>
                                </CTableRow>
                              ))}
                            </CTableBody>
                          </CTable>
                        </CCardBody>
                      </CCard>
                    </CCol>
                  </CRow>
                </>
              )}
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>
    </CContainer>
  );
};

export default ResourceCostHistory;