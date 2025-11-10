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


import React from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CBadge,
  CProgress,
  CProgressBar,
  CAlert
} from "@coreui/react";
import { CChart } from "@coreui/react-chartjs";

const DatabaseOverview = ({ connection, analysisResult, performanceMetrics }) => {
  return (
    <div>
      <CRow className="mb-4">
        <CCol lg={3} md={6}>
          <CCard className="text-center">
            <CCardBody>
              <div className="fs-4 fw-semibold">⚡</div>
              <div className="text-medium-emphasis text-uppercase fw-semibold small">CPU Usage</div>
              <div className="fs-6 fw-semibold text-primary">
                {performanceMetrics?.compute_metrics?.cpu_usage?.toFixed(1) || 0}%
              </div>
            </CCardBody>
          </CCard>
        </CCol>
        <CCol lg={3} md={6}>
          <CCard className="text-center">
            <CCardBody>
              <div className="fs-4 fw-semibold">🧠</div>
              <div className="text-medium-emphasis text-uppercase fw-semibold small">Memory Usage</div>
              <div className="fs-6 fw-semibold text-info">
                {performanceMetrics?.compute_metrics?.memory_usage_bytes ? 
                  (performanceMetrics.compute_metrics.memory_usage_bytes / (1024**3)).toFixed(1) + ' GB' : 'N/A'}
              </div>
            </CCardBody>
          </CCard>
        </CCol>
        <CCol lg={3} md={6}>
          <CCard className="text-center">
            <CCardBody>
              <div className="fs-4 fw-semibold">🔗</div>
              <div className="text-medium-emphasis text-uppercase fw-semibold small">Connections</div>
              <div className="fs-6 fw-semibold text-success">
                {performanceMetrics?.performance_metrics?.active_sessions || 0} / {performanceMetrics?.performance_metrics?.connection_count || 0}
              </div>
            </CCardBody>
          </CCard>
        </CCol>
        <CCol lg={3} md={6}>
          <CCard className="text-center">
            <CCardBody>
              <div className="fs-4 fw-semibold">📊</div>
              <div className="text-medium-emphasis text-uppercase fw-semibold small">Buffer Hit Ratio</div>
              <div className="fs-6 fw-semibold text-warning">
                {performanceMetrics?.performance_metrics?.buffer_hit_ratio ? 
                  (performanceMetrics.performance_metrics.buffer_hit_ratio * 100).toFixed(1) + '%' : 'N/A'}
              </div>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      <CRow>
        <CCol lg={8}>
          <CCard className="mb-4">
            <CCardHeader>
              <strong>Connection Details</strong>
            </CCardHeader>
            <CCardBody>
              <CRow>
                <CCol sm={3}><strong>Host:</strong></CCol>
                <CCol sm={9}>{connection.host}:{connection.port}</CCol>
              </CRow>
              <CRow>
                <CCol sm={3}><strong>Database:</strong></CCol>
                <CCol sm={9}>{connection.database_name || 'N/A'}</CCol>
              </CRow>
              <CRow>
                <CCol sm={3}><strong>Username:</strong></CCol>
                <CCol sm={9}>{connection.username || 'N/A'}</CCol>
              </CRow>
              <CRow>
                <CCol sm={3}><strong>SSL Mode:</strong></CCol>
                <CCol sm={9}>
                  <CBadge color={connection.ssl_mode === 'require' ? 'success' : 'secondary'}>
                    {connection.ssl_mode || 'disabled'}
                  </CBadge>
                </CCol>
              </CRow>
              <CRow>
                <CCol sm={3}><strong>Status:</strong></CCol>
                <CCol sm={9}>
                  <CBadge color={connection.connection_status === 'active' ? 'success' : 'warning'}>
                    {connection.connection_status || 'unknown'}
                  </CBadge>
                </CCol>
              </CRow>
            </CCardBody>
          </CCard>
        </CCol>

        <CCol lg={4}>
          <CCard className="mb-4">
            <CCardHeader>
              <strong>Performance Summary</strong>
            </CCardHeader>
            <CCardBody>
              {performanceMetrics?.performance_metrics ? (
                <div>
                  <div className="mb-3">
                    <div className="text-medium-emphasis small">Buffer Hit Ratio</div>
                    <CProgress className="mb-1">
                      <CProgressBar 
                        value={performanceMetrics.performance_metrics.buffer_hit_ratio * 100}
                        color="success"
                      />
                    </CProgress>
                    <small>{(performanceMetrics.performance_metrics.buffer_hit_ratio * 100).toFixed(1)}%</small>
                  </div>

                  <div className="mb-3">
                    <div className="text-medium-emphasis small">Cache Hit Ratio</div>
                    <CProgress className="mb-1">
                      <CProgressBar 
                        value={performanceMetrics.performance_metrics.cache_hit_ratio * 100}
                        color="info"
                      />
                    </CProgress>
                    <small>{(performanceMetrics.performance_metrics.cache_hit_ratio * 100).toFixed(1)}%</small>
                  </div>

                  <div className="row">
                    <div className="col-6">
                      <div className="text-medium-emphasis small">Deadlocks</div>
                      <div className="fw-semibold">{performanceMetrics.performance_metrics.deadlocks}</div>
                    </div>
                    <div className="col-6">
                      <div className="text-medium-emphasis small">Blocked Queries</div>
                      <div className="fw-semibold">{performanceMetrics.performance_metrics.blocked_queries}</div>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="text-center text-muted">
                  <div>No performance data available</div>
                  <small>Run analysis to see metrics</small>
                </div>
              )}
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      {analysisResult?.issues && analysisResult.issues.length > 0 && (
        <CRow>
          <CCol>
            <CCard>
              <CCardHeader>
                <strong>Database Issues</strong>
              </CCardHeader>
              <CCardBody>
                {analysisResult.issues.map((issue, index) => (
                  <CAlert key={index} color={getSeverityColor(issue.severity)} className="mb-2">
                    <div className="d-flex justify-content-between align-items-start">
                      <div>
                        <strong>{issue.title}</strong>
                        <div className="mt-1">{issue.description}</div>
                        <div className="mt-2 small text-muted">
                          <strong>Recommendation:</strong> {issue.recommendation}
                        </div>
                        {issue.affected_objects && issue.affected_objects.length > 0 && (
                          <div className="mt-2">
                            <strong>Affected objects:</strong> {issue.affected_objects.join(', ')}
                          </div>
                        )}
                      </div>
                      <CBadge color={getSeverityColor(issue.severity)}>
                        {issue.severity}
                      </CBadge>
                    </div>
                  </CAlert>
                ))}
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}
    </div>
  );
};

const getSeverityColor = (severity) => {
  const colors = {
    Critical: 'danger',
    High: 'warning', 
    Medium: 'info',
    Low: 'secondary',
    Info: 'light'
  };
  return colors[severity] || 'secondary';
};

export default DatabaseOverview;
