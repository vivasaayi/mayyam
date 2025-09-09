import React, { useState, useEffect, useRef } from 'react';
import {
  CCard,
  CCardHeader,
  CCardBody,
  CRow,
  CCol,
  CFormSelect,
  CButton,
  CAlert,
  CSpinner,
  CBadge,
  CButtonGroup
} from '@coreui/react';
import { FaChartLine, FaRefresh, FaPlay, FaStop } from 'react-icons/fa';
import Chart from 'chart.js/auto';
import KinesisService from '../../services/kinesisService';

const StreamMetricsChart = ({ profile, region, streamName }) => {
  const [metrics, setMetrics] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [timeRange, setTimeRange] = useState('1h');
  const [metricType, setMetricType] = useState('IncomingRecords');
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [refreshInterval, setRefreshInterval] = useState(null);
  
  const chartRef = useRef(null);
  const chartInstance = useRef(null);

  // Available metrics
  const availableMetrics = [
    { value: 'IncomingRecords', label: 'Incoming Records', unit: 'count' },
    { value: 'IncomingBytes', label: 'Incoming Bytes', unit: 'bytes' },
    { value: 'OutgoingRecords', label: 'Outgoing Records', unit: 'count' },
    { value: 'OutgoingBytes', label: 'Outgoing Bytes', unit: 'bytes' },
    { value: 'WriteProvisionedThroughputExceeded', label: 'Write Throttled', unit: 'count' },
    { value: 'ReadProvisionedThroughputExceeded', label: 'Read Throttled', unit: 'count' },
    { value: 'IteratorAgeMilliseconds', label: 'Iterator Age', unit: 'milliseconds' }
  ];

  // Time range options
  const timeRanges = [
    { value: '15m', label: 'Last 15 minutes', minutes: 15 },
    { value: '1h', label: 'Last 1 hour', minutes: 60 },
    { value: '6h', label: 'Last 6 hours', minutes: 360 },
    { value: '12h', label: 'Last 12 hours', minutes: 720 },
    { value: '24h', label: 'Last 24 hours', minutes: 1440 }
  ];

  // Load metrics data
  const loadMetrics = async () => {
    setLoading(true);
    setError(null);
    
    try {
      const selectedRange = timeRanges.find(r => r.value === timeRange);
      const endTime = new Date();
      const startTime = new Date(endTime.getTime() - (selectedRange.minutes * 60 * 1000));

      const metricsData = await KinesisService.getCloudWatchMetrics(
        profile,
        region,
        streamName,
        metricType,
        startTime.toISOString(),
        endTime.toISOString(),
        selectedRange.minutes < 60 ? 60 : 300 // 1 minute for short ranges, 5 minutes for longer
      );

      setMetrics(metricsData.datapoints || []);
      renderChart(metricsData.datapoints || []);
    } catch (err) {
      setError(`Failed to load metrics: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Render chart
  const renderChart = (data) => {
    if (!chartRef.current) return;

    // Destroy existing chart
    if (chartInstance.current) {
      chartInstance.current.destroy();
    }

    const ctx = chartRef.current.getContext('2d');
    
    // Prepare data for Chart.js
    const sortedData = data.sort((a, b) => new Date(a.timestamp) - new Date(b.timestamp));
    const labels = sortedData.map(point => new Date(point.timestamp).toLocaleTimeString());
    const values = sortedData.map(point => point.value || 0);

    const selectedMetric = availableMetrics.find(m => m.value === metricType);
    
    chartInstance.current = new Chart(ctx, {
      type: 'line',
      data: {
        labels: labels,
        datasets: [{
          label: selectedMetric.label,
          data: values,
          borderColor: 'rgb(75, 192, 192)',
          backgroundColor: 'rgba(75, 192, 192, 0.2)',
          tension: 0.1,
          fill: true
        }]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        scales: {
          y: {
            beginAtZero: true,
            title: {
              display: true,
              text: selectedMetric.unit
            }
          },
          x: {
            title: {
              display: true,
              text: 'Time'
            }
          }
        },
        plugins: {
          legend: {
            display: true,
            position: 'top'
          },
          title: {
            display: true,
            text: `${selectedMetric.label} - ${streamName}`
          }
        },
        interaction: {
          intersect: false,
          mode: 'index'
        }
      }
    });
  };

  // Toggle auto-refresh
  const toggleAutoRefresh = () => {
    if (autoRefresh) {
      if (refreshInterval) {
        clearInterval(refreshInterval);
        setRefreshInterval(null);
      }
      setAutoRefresh(false);
    } else {
      const interval = setInterval(loadMetrics, 30000); // Refresh every 30 seconds
      setRefreshInterval(interval);
      setAutoRefresh(true);
    }
  };

  // Load metrics when component mounts or parameters change
  useEffect(() => {
    loadMetrics();
    
    // Cleanup interval on unmount
    return () => {
      if (refreshInterval) {
        clearInterval(refreshInterval);
      }
      if (chartInstance.current) {
        chartInstance.current.destroy();
      }
    };
  }, [streamName, metricType, timeRange]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (refreshInterval) {
        clearInterval(refreshInterval);
      }
      if (chartInstance.current) {
        chartInstance.current.destroy();
      }
    };
  }, []);

  // Calculate summary statistics
  const calculateStats = () => {
    if (metrics.length === 0) return null;

    const values = metrics.map(m => m.value || 0);
    const sum = values.reduce((a, b) => a + b, 0);
    const avg = sum / values.length;
    const max = Math.max(...values);
    const min = Math.min(...values);

    return { sum, avg, max, min, count: values.length };
  };

  const stats = calculateStats();
  const selectedMetric = availableMetrics.find(m => m.value === metricType);

  return (
    <div>
      {/* Controls */}
      <CRow className="mb-3">
        <CCol md={3}>
          <CFormSelect
            label="Metric"
            value={metricType}
            onChange={(e) => setMetricType(e.target.value)}
          >
            {availableMetrics.map(metric => (
              <option key={metric.value} value={metric.value}>
                {metric.label}
              </option>
            ))}
          </CFormSelect>
        </CCol>
        <CCol md={3}>
          <CFormSelect
            label="Time Range"
            value={timeRange}
            onChange={(e) => setTimeRange(e.target.value)}
          >
            {timeRanges.map(range => (
              <option key={range.value} value={range.value}>
                {range.label}
              </option>
            ))}
          </CFormSelect>
        </CCol>
        <CCol md={6} className="d-flex align-items-end">
          <CButtonGroup>
            <CButton
              color="primary"
              onClick={loadMetrics}
              disabled={loading}
            >
              {loading ? <CSpinner size="sm" /> : <FaRefresh />}
              {loading ? ' Loading...' : ' Refresh'}
            </CButton>
            <CButton
              color={autoRefresh ? "success" : "outline-success"}
              onClick={toggleAutoRefresh}
            >
              {autoRefresh ? <FaStop /> : <FaPlay />}
              {autoRefresh ? ' Stop Auto-refresh' : ' Auto-refresh'}
            </CButton>
          </CButtonGroup>
        </CCol>
      </CRow>

      {/* Error Alert */}
      {error && (
        <CAlert color="danger" className="mb-3">
          {error}
        </CAlert>
      )}

      {/* Statistics Summary */}
      {stats && (
        <CRow className="mb-3">
          <CCol>
            <CCard>
              <CCardHeader>
                Statistics - {selectedMetric.label}
                {autoRefresh && <CBadge color="success" className="ms-2">Live</CBadge>}
              </CCardHeader>
              <CCardBody>
                <CRow>
                  <CCol md={3}>
                    <div className="text-center">
                      <h5 className="text-primary">{stats.sum.toLocaleString()}</h5>
                      <small className="text-muted">Total</small>
                    </div>
                  </CCol>
                  <CCol md={3}>
                    <div className="text-center">
                      <h5 className="text-info">{stats.avg.toFixed(2)}</h5>
                      <small className="text-muted">Average</small>
                    </div>
                  </CCol>
                  <CCol md={3}>
                    <div className="text-center">
                      <h5 className="text-success">{stats.max.toLocaleString()}</h5>
                      <small className="text-muted">Maximum</small>
                    </div>
                  </CCol>
                  <CCol md={3}>
                    <div className="text-center">
                      <h5 className="text-warning">{stats.min.toLocaleString()}</h5>
                      <small className="text-muted">Minimum</small>
                    </div>
                  </CCol>
                </CRow>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}

      {/* Chart */}
      <CCard>
        <CCardHeader>
          <FaChartLine className="me-2" />
          Metrics Chart - {streamName}
        </CCardHeader>
        <CCardBody>
          <div style={{ position: 'relative', height: '400px' }}>
            <canvas ref={chartRef}></canvas>
          </div>
          {metrics.length === 0 && !loading && (
            <div className="text-center text-muted py-5">
              No metric data available for the selected time range
            </div>
          )}
        </CCardBody>
      </CCard>

      {/* Data Points Summary */}
      {metrics.length > 0 && (
        <CCard className="mt-3">
          <CCardHeader>Recent Data Points</CCardHeader>
          <CCardBody>
            <div style={{ maxHeight: '200px', overflowY: 'auto' }}>
              {metrics
                .sort((a, b) => new Date(b.timestamp) - new Date(a.timestamp))
                .slice(0, 10)
                .map((point, index) => (
                  <div key={index} className="d-flex justify-content-between border-bottom py-1">
                    <span>{new Date(point.timestamp).toLocaleString()}</span>
                    <strong>{(point.value || 0).toLocaleString()} {selectedMetric.unit}</strong>
                  </div>
                ))}
            </div>
          </CCardBody>
        </CCard>
      )}
    </div>
  );
};

export default StreamMetricsChart;
