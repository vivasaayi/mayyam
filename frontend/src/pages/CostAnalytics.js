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
import { CChartLine, CChartBar } from '@coreui/react-chartjs';
import { apiCall } from '../services/api';

const CostAnalytics = () => {
  const [loading, setLoading] = useState(false);
  const [costData, setCostData] = useState(null);
  const [anomalies, setAnomalies] = useState([]);
  const [insights, setInsights] = useState([]);
  const [error, setError] = useState(null);
  const [selectedAnomaly, setSelectedAnomaly] = useState(null);
  const [analyzing, setAnalyzing] = useState(false);

  // Form state
  const [formData, setFormData] = useState({
    accountId: '',
    startDate: '',
    endDate: '',
    granularity: 'MONTHLY'
  });

  const fetchCostData = async () => {
    if (!formData.accountId || !formData.startDate || !formData.endDate) {
      setError('Please fill in all required fields');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const params = new URLSearchParams({
        account_id: formData.accountId,
        start_date: formData.startDate,
        end_date: formData.endDate,
        granularity: formData.granularity
      });

      const response = await apiCall(`/api/cost-analytics/fetch?${params}`);
      setCostData(response.data);

      // Fetch anomalies and insights
      await fetchAnomalies();
      await fetchInsights();
    } catch (err) {
      setError(err.message || 'Failed to fetch cost data');
    } finally {
      setLoading(false);
    }
  };

  const fetchAnomalies = async () => {
    try {
      const response = await apiCall(`/api/cost-analytics/anomalies?account_id=${formData.accountId}`);
      setAnomalies(response.data.anomalies || []);
    } catch (err) {
      console.error('Failed to fetch anomalies:', err);
    }
  };

  const fetchInsights = async () => {
    try {
      const response = await apiCall(`/api/cost-analytics/insights?account_id=${formData.accountId}`);
      setInsights(response.data.insights || []);
    } catch (err) {
      console.error('Failed to fetch insights:', err);
    }
  };

  const analyzeAnomaly = async (anomalyId) => {
    setAnalyzing(true);
    try {
      const response = await apiCall(`/api/cost-analytics/analyze/${anomalyId}`, 'POST');
      setSelectedAnomaly({
        ...selectedAnomaly,
        analysis: response.data
      });
      // Refresh insights
      await fetchInsights();
    } catch (err) {
      setError('Failed to analyze anomaly');
    } finally {
      setAnalyzing(false);
    }
  };

  const getSeverityColor = (severity) => {
    switch (severity?.toLowerCase()) {
      case 'critical': return 'danger';
      case 'high': return 'warning';
      case 'medium': return 'info';
      case 'low': return 'secondary';
      default: return 'secondary';
    }
  };

  return (
    <CContainer fluid>
      <div className="mb-4">
        <h1>AWS Cost Analytics</h1>
        <p className="text-muted">Analyze AWS costs, detect anomalies, and get AI-powered insights</p>
      </div>

      {/* Cost Data Form */}
      <CCard className="mb-4">
        <CCardHeader>
          <CCardTitle>Fetch Cost Data</CCardTitle>
        </CCardHeader>
        <CCardBody>
          <CForm className="row g-3">
            <CCol md={3}>
              <CFormInput
                type="text"
                label="AWS Account ID"
                placeholder="123456789012"
                value={formData.accountId}
                onChange={(e) => setFormData({...formData, accountId: e.target.value})}
                required
              />
            </CCol>
            <CCol md={2}>
              <CFormInput
                type="date"
                label="Start Date"
                value={formData.startDate}
                onChange={(e) => setFormData({...formData, startDate: e.target.value})}
                required
              />
            </CCol>
            <CCol md={2}>
              <CFormInput
                type="date"
                label="End Date"
                value={formData.endDate}
                onChange={(e) => setFormData({...formData, endDate: e.target.value})}
                required
              />
            </CCol>
            <CCol md={2}>
              <CFormSelect
                label="Granularity"
                value={formData.granularity}
                onChange={(e) => setFormData({...formData, granularity: e.target.value})}
              >
                <option value="DAILY">Daily</option>
                <option value="MONTHLY">Monthly</option>
              </CFormSelect>
            </CCol>
            <CCol md={3}>
              <div className="d-grid">
                <CButton
                  color="primary"
                  onClick={fetchCostData}
                  disabled={loading}
                  className="mt-4"
                >
                  {loading ? <CSpinner size="sm" /> : 'Fetch Cost Data'}
                </CButton>
              </div>
            </CCol>
          </CForm>
        </CCardBody>
      </CCard>

      {error && (
        <CAlert color="danger" className="mb-4">
          {error}
        </CAlert>
      )}

      {/* Cost Overview */}
      {costData && (
        <CRow className="mb-4">
          <CCol md={4}>
            <CCard>
              <CCardBody>
                <div className="text-center">
                  <h2 className="text-primary">${costData.total_cost?.toFixed(2) || '0.00'}</h2>
                  <p className="text-muted">Total Cost</p>
                </div>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={4}>
            <CCard>
              <CCardBody>
                <div className="text-center">
                  <h2 className="text-warning">{anomalies.length}</h2>
                  <p className="text-muted">Cost Anomalies</p>
                </div>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={4}>
            <CCard>
              <CCardBody>
                <div className="text-center">
                  <h2 className="text-info">{insights.length}</h2>
                  <p className="text-muted">AI Insights</p>
                </div>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Charts */}
      {costData && (
        <CRow className="mb-4">
          <CCol md={6}>
            <CCard>
              <CCardHeader>
                <CCardTitle>Monthly Cost Trend</CCardTitle>
              </CCardHeader>
              <CCardBody>
                <CChartLine
                  data={{
                    labels: costData.monthly_trend?.map(item => item[0]) || [],
                    datasets: [{
                      label: 'Cost ($)',
                      data: costData.monthly_trend?.map(item => item[1]) || [],
                      borderColor: '#321fdb',
                      backgroundColor: 'rgba(50, 31, 219, 0.1)',
                    }]
                  }}
                  options={{
                    responsive: true,
                    scales: {
                      y: {
                        beginAtZero: true,
                        ticks: {
                          callback: (value) => `$${value}`
                        }
                      }
                    }
                  }}
                />
              </CCardBody>
            </CCard>
          </CCol>
          <CCol md={6}>
            <CCard>
              <CCardHeader>
                <CCardTitle>Cost by Service</CCardTitle>
              </CCardHeader>
              <CCardBody>
                <CChartBar
                  data={{
                    labels: Object.keys(costData.service_breakdown || {}),
                    datasets: [{
                      label: 'Cost ($)',
                      data: Object.values(costData.service_breakdown || {}),
                      backgroundColor: '#321fdb',
                    }]
                  }}
                  options={{
                    responsive: true,
                    scales: {
                      y: {
                        beginAtZero: true,
                        ticks: {
                          callback: (value) => `$${value}`
                        }
                      }
                    }
                  }}
                />
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Anomalies Table */}
      {anomalies.length > 0 && (
        <CCard className="mb-4">
          <CCardHeader>
            <CCardTitle>Cost Anomalies</CCardTitle>
          </CCardHeader>
          <CCardBody>
            <CTable hover>
              <CTableHead>
                <CTableRow>
                  <CTableHeaderCell>Service</CTableHeaderCell>
                  <CTableHeaderCell>Type</CTableHeaderCell>
                  <CTableHeaderCell>Severity</CTableHeaderCell>
                  <CTableHeaderCell>Cost Change</CTableHeaderCell>
                  <CTableHeaderCell>Description</CTableHeaderCell>
                  <CTableHeaderCell>Actions</CTableHeaderCell>
                </CTableRow>
              </CTableHead>
              <CTableBody>
                {anomalies.map((anomaly, index) => (
                  <CTableRow key={index}>
                    <CTableDataCell>{anomaly.service_name}</CTableDataCell>
                    <CTableDataCell>{anomaly.anomaly_type}</CTableDataCell>
                    <CTableDataCell>
                      <CBadge color={getSeverityColor(anomaly.severity)}>
                        {anomaly.severity}
                      </CBadge>
                    </CTableDataCell>
                    <CTableDataCell>
                      {anomaly.percentage_change > 0 ? '+' : ''}{anomaly.percentage_change?.toFixed(1)}%
                    </CTableDataCell>
                    <CTableDataCell>{anomaly.description}</CTableDataCell>
                    <CTableDataCell>
                      <CButton
                        size="sm"
                        color="primary"
                        onClick={() => setSelectedAnomaly(anomaly)}
                      >
                        Analyze
                      </CButton>
                    </CTableDataCell>
                  </CTableRow>
                ))}
              </CTableBody>
            </CTable>
          </CCardBody>
        </CCard>
      )}

      {/* Anomaly Analysis Modal */}
      <CModal visible={!!selectedAnomaly} onClose={() => setSelectedAnomaly(null)} size="lg">
        <CModalHeader>
          <CModalTitle>AI Analysis: {selectedAnomaly?.service_name}</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {selectedAnomaly && (
            <div>
              <div className="mb-3">
                <strong>Anomaly Details:</strong>
                <p>Type: {selectedAnomaly.anomaly_type}</p>
                <p>Severity: <CBadge color={getSeverityColor(selectedAnomaly.severity)}>{selectedAnomaly.severity}</CBadge></p>
                <p>Cost Change: {selectedAnomaly.percentage_change?.toFixed(1)}%</p>
                <p>Description: {selectedAnomaly.description}</p>
              </div>

              {selectedAnomaly.analysis ? (
                <div>
                  <strong>AI Analysis:</strong>
                  <div className="mt-2 p-3 bg-light rounded">
                    <pre style={{ whiteSpace: 'pre-wrap', fontFamily: 'inherit' }}>
                      {JSON.stringify(selectedAnomaly.analysis, null, 2)}
                    </pre>
                  </div>
                </div>
              ) : (
                <div className="text-center">
                  <CButton
                    color="primary"
                    onClick={() => analyzeAnomaly(selectedAnomaly.id)}
                    disabled={analyzing}
                  >
                    {analyzing ? <CSpinner size="sm" /> : 'Generate AI Analysis'}
                  </CButton>
                </div>
              )}
            </div>
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setSelectedAnomaly(null)}>
            Close
          </CButton>
        </CModalFooter>
      </CModal>
    </CContainer>
  );
};

export default CostAnalytics;