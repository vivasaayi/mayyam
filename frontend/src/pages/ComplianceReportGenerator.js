import React, { useState, useEffect } from 'react';
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CForm,
  CFormInput,
  CFormSelect,
  CAlert,
  CSpinner,
  CModal,
  CModalHeader,
  CModalBody,
  CModalFooter,
  CProgress,
} from '@coreui/react';
import CIcon from '@coreui/icons-react';
import { cilCloudDownload, cilCheckAlt, cilWarning } from '@coreui/icons';
import auditService from '../services/auditService';
import metricsService from '../services/metricsService';

const ComplianceReportGenerator = () => {
  const [loading, setLoading] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(false);
  const [showPreview, setShowPreview] = useState(false);
  const [reportData, setReportData] = useState(null);

  const [reportConfig, setReportConfig] = useState({
    title: 'Chaos Engineering Compliance Report',
    organization: '',
    reportPeriod: 'monthly',
    startDate: '',
    endDate: '',
    includeMetrics: true,
    includeAuditTrail: true,
    includeRiskAssessment: true,
    customNotes: '',
  });

  const generateReport = async () => {
    setGenerating(true);
    setError(null);
    try {
      // Fetch audit logs
      const auditLogs = await auditService.listAuditLogs({
        start_date: reportConfig.startDate,
        end_date: reportConfig.endDate,
        page_size: 10000,
      });

      // Fetch metrics
      const metrics = await metricsService.getMetricsStats({
        start_date: reportConfig.startDate,
        end_date: reportConfig.endDate,
      });

      // Calculate risk assessment
      const riskAssessment = calculateRiskAssessment(metrics, auditLogs.logs);

      setReportData({
        config: reportConfig,
        auditLogs: auditLogs.logs,
        metrics: metricsService.formatMetrics(metrics),
        riskAssessment,
        generatedAt: new Date().toISOString(),
      });

      setSuccess(true);
      setShowPreview(true);
    } catch (err) {
      setError(err.response?.data?.message || 'Failed to generate report');
    } finally {
      setGenerating(false);
    }
  };

  const calculateRiskAssessment = (metrics, auditLogs) => {
    const assessment = {
      overallRisk: 'LOW',
      riskScore: 0,
      findings: [],
    };

    // Analyze success rate
    if (metrics.success_rate_percent < 80) {
      assessment.riskScore += 30;
      assessment.findings.push({
        severity: 'MEDIUM',
        finding: `Success rate is ${metrics.success_rate_percent}%, below 80% threshold`,
      });
    }

    // Analyze rollback success
    if (metrics.rollback_success_rate_percent < 95) {
      assessment.riskScore += 20;
      assessment.findings.push({
        severity: 'MEDIUM',
        finding: `Rollback success rate is ${metrics.rollback_success_rate_percent}%, below 95% threshold`,
      });
    }

    // Analyze recovery time
    if (metrics.avg_recovery_time_ms > 300000) {
      assessment.riskScore += 15;
      assessment.findings.push({
        severity: 'LOW',
        finding: `Average recovery time is ${Math.round(metrics.avg_recovery_time_ms / 1000)}s, exceeds 5 minute baseline`,
      });
    }

    // Analyze audit trail completeness
    const auditCoverage = auditLogs.filter(log => log.user_id || log.ip_address).length / auditLogs.length;
    if (auditCoverage < 0.95) {
      assessment.riskScore += 10;
      assessment.findings.push({
        severity: 'LOW',
        finding: `${Math.round(auditCoverage * 100)}% of audit logs have user/IP context`,
      });
    }

    // Determine overall risk
    if (assessment.riskScore >= 50) assessment.overallRisk = 'HIGH';
    else if (assessment.riskScore >= 25) assessment.overallRisk = 'MEDIUM';

    return assessment;
  };

  const downloadReport = () => {
    if (!reportData) return;

    const html = generateHTMLReport(reportData);
    const blob = new Blob([html], { type: 'text/html' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `compliance-report-${new Date().toISOString().split('T')[0]}.html`;
    a.click();
    window.URL.revokeObjectURL(url);
  };

  const downloadPDF = () => {
    if (!reportData) return;

    // For actual PDF generation, you would integrate a library like jsPDF
    // For now, trigger HTML download as a workaround
    downloadReport();
  };

  const downloadCSV = () => {
    if (!reportData) return;

    const csv = generateCSVReport(reportData);
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `compliance-report-${new Date().toISOString().split('T')[0]}.csv`;
    a.click();
    window.URL.revokeObjectURL(url);
  };

  const generateHTMLReport = (data) => {
    const { config, metrics, riskAssessment, auditLogs } = data;

    return `
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>${config.title}</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; color: #333; }
        h1 { color: #007bff; border-bottom: 2px solid #007bff; padding-bottom: 10px; }
        h2 { color: #495057; margin-top: 30px; }
        .header { background: #f8f9fa; padding: 20px; border-radius: 5px; margin-bottom: 20px; }
        .metrics { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; margin: 20px 0; }
        .metric-card { background: #f8f9fa; padding: 15px; border-radius: 5px; border-left: 4px solid #007bff; }
        .metric-value { font-size: 24px; font-weight: bold; color: #007bff; }
        .risk-high { color: #dc3545; }
        .risk-medium { color: #ffc107; }
        .risk-low { color: #28a745; }
        table { width: 100%; border-collapse: collapse; margin: 20px 0; }
        th, td { padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }
        th { background: #007bff; color: white; }
        tr:nth-child(even) { background: #f8f9fa; }
        .footer { margin-top: 40px; padding-top: 20px; border-top: 1px solid #ddd; font-size: 12px; color: #666; }
    </style>
</head>
<body>
    <h1>${config.title}</h1>

    <div class="header">
        <p><strong>Organization:</strong> ${config.organization || 'N/A'}</p>
        <p><strong>Report Period:</strong> ${config.reportPeriod}</p>
        <p><strong>Generated:</strong> ${new Date(data.generatedAt).toLocaleString()}</p>
    </div>

    <h2>Executive Summary</h2>
    <p>This compliance report provides a comprehensive overview of chaos engineering activities, audit logs, and metrics for the specified period.</p>

    <h2>Risk Assessment</h2>
    <p><strong>Overall Risk Level:</strong> <span class="risk-${riskAssessment.overallRisk.toLowerCase()}">${riskAssessment.overallRisk}</span></p>
    <p><strong>Risk Score:</strong> ${riskAssessment.riskScore}/100</p>

    ${riskAssessment.findings.length > 0 ? `
    <h3>Findings</h3>
    <ul>
        ${riskAssessment.findings.map(f => `<li>[${f.severity}] ${f.finding}</li>`).join('')}
    </ul>
    ` : '<p>No significant findings.</p>'}

    ${config.includeMetrics ? `
    <h2>Metrics Summary</h2>
    <div class="metrics">
        <div class="metric-card">
            <div>Total Experiments</div>
            <div class="metric-value">${metrics.totalExperiments}</div>
        </div>
        <div class="metric-card">
            <div>Success Rate</div>
            <div class="metric-value">${metrics.successRate}%</div>
        </div>
        <div class="metric-card">
            <div>Avg Execution Time</div>
            <div class="metric-value">${metrics.avgExecutionDuration}ms</div>
        </div>
        <div class="metric-card">
            <div>Rollback Success</div>
            <div class="metric-value">${metrics.rollbackSuccessRate}%</div>
        </div>
    </div>
    ` : ''}

    ${config.includeAuditTrail && auditLogs.length > 0 ? `
    <h2>Recent Audit Trail (Last 50 entries)</h2>
    <table>
        <thead>
            <tr>
                <th>Timestamp</th>
                <th>Action</th>
                <th>User</th>
                <th>Resource</th>
                <th>Status</th>
            </tr>
        </thead>
        <tbody>
            ${auditLogs.slice(0, 50).map(log => `
            <tr>
                <td>${new Date(log.created_at).toLocaleString()}</td>
                <td>${log.action}</td>
                <td>${log.user_id || '-'}</td>
                <td>${log.resource_id || '-'}</td>
                <td>${log.status_before ? log.status_after : '-'}</td>
            </tr>
            `).join('')}
        </tbody>
    </table>
    ` : ''}

    ${config.customNotes ? `
    <h2>Notes</h2>
    <p>${config.customNotes}</p>
    ` : ''}

    <div class="footer">
        <p>This is an automated compliance report generated by Mayyam Chaos Engineering Platform.</p>
        <p>For questions or concerns, please contact your security/compliance team.</p>
    </div>
</body>
</html>
    `;
  };

  const generateCSVReport = (data) => {
    const { auditLogs, metrics } = data;

    let csv = 'Chaos Engineering Compliance Report\n';
    csv += `Generated: ${new Date(data.generatedAt).toLocaleString()}\n\n`;

    csv += 'METRICS SUMMARY\n';
    csv += `Total Experiments,${metrics.totalExperiments}\n`;
    csv += `Success Rate,${metrics.successRate}%\n`;
    csv += `Avg Execution Duration,${metrics.avgExecutionDuration}ms\n`;
    csv += `Rollback Success Rate,${metrics.rollbackSuccessRate}%\n\n`;

    csv += 'AUDIT LOGS\n';
    csv += 'Timestamp,Action,User ID,Triggered By,Resource ID,Status Before,Status After\n';
    auditLogs.forEach(log => {
      csv += `"${new Date(log.created_at).toLocaleString()}","${log.action}","${log.user_id || ''}","${log.triggered_by || ''}","${log.resource_id || ''}","${log.status_before || ''}","${log.status_after || ''}"\n`;
    });

    return csv;
  };

  const handleConfigChange = (field, value) => {
    setReportConfig(prev => ({
      ...prev,
      [field]: value,
    }));
  };

  return (
    <CRow className="mb-3">
      <CCol xs={12}>
        <CCard>
          <CCardHeader>
            <span>Compliance Report Generator</span>
          </CCardHeader>
          <CCardBody>
            {error && <CAlert color="danger" className="mb-3" dismissible onClose={() => setError(null)}>{error}</CAlert>}
            {success && <CAlert color="success" className="mb-3" dismissible onClose={() => setSuccess(false)}>Report generated successfully!</CAlert>}

            <CForm className="mb-3 p-3 bg-light rounded">
              <h6 className="mb-3">Report Configuration</h6>
              <CRow>
                <CCol md={6} className="mb-3">
                  <label className="form-label small">Report Title</label>
                  <CFormInput
                    size="sm"
                    value={reportConfig.title}
                    onChange={(e) => handleConfigChange('title', e.target.value)}
                  />
                </CCol>

                <CCol md={6} className="mb-3">
                  <label className="form-label small">Organization</label>
                  <CFormInput
                    size="sm"
                    value={reportConfig.organization}
                    onChange={(e) => handleConfigChange('organization', e.target.value)}
                  />
                </CCol>

                <CCol md={6} className="mb-3">
                  <label className="form-label small">Report Period</label>
                  <CFormSelect
                    size="sm"
                    value={reportConfig.reportPeriod}
                    onChange={(e) => handleConfigChange('reportPeriod', e.target.value)}
                  >
                    <option value="daily">Daily</option>
                    <option value="weekly">Weekly</option>
                    <option value="monthly">Monthly</option>
                    <option value="quarterly">Quarterly</option>
                    <option value="custom">Custom</option>
                  </CFormSelect>
                </CCol>

                <CCol md={6} className="mb-3">
                  <label className="form-label small">Start Date</label>
                  <CFormInput
                    size="sm"
                    type="datetime-local"
                    value={reportConfig.startDate}
                    onChange={(e) => handleConfigChange('startDate', e.target.value)}
                  />
                </CCol>

                <CCol md={6} className="mb-3">
                  <label className="form-label small">End Date</label>
                  <CFormInput
                    size="sm"
                    type="datetime-local"
                    value={reportConfig.endDate}
                    onChange={(e) => handleConfigChange('endDate', e.target.value)}
                  />
                </CCol>

                <CCol md={12} className="mb-3">
                  <label className="form-label small">Additional Notes</label>
                  <textarea
                    className="form-control form-control-sm"
                    rows="3"
                    value={reportConfig.customNotes}
                    onChange={(e) => handleConfigChange('customNotes', e.target.value)}
                    placeholder="Add any additional notes or context for the report..."
                  />
                </CCol>

                <CCol md={12} className="mb-3">
                  <div className="form-check">
                    <input
                      className="form-check-input"
                      type="checkbox"
                      id="includeMetrics"
                      checked={reportConfig.includeMetrics}
                      onChange={(e) => handleConfigChange('includeMetrics', e.target.checked)}
                    />
                    <label className="form-check-label" htmlFor="includeMetrics">
                      Include Metrics Summary
                    </label>
                  </div>
                  <div className="form-check">
                    <input
                      className="form-check-input"
                      type="checkbox"
                      id="includeAuditTrail"
                      checked={reportConfig.includeAuditTrail}
                      onChange={(e) => handleConfigChange('includeAuditTrail', e.target.checked)}
                    />
                    <label className="form-check-label" htmlFor="includeAuditTrail">
                      Include Audit Trail
                    </label>
                  </div>
                  <div className="form-check">
                    <input
                      className="form-check-input"
                      type="checkbox"
                      id="includeRiskAssessment"
                      checked={reportConfig.includeRiskAssessment}
                      onChange={(e) => handleConfigChange('includeRiskAssessment', e.target.checked)}
                    />
                    <label className="form-check-label" htmlFor="includeRiskAssessment">
                      Include Risk Assessment
                    </label>
                  </div>
                </CCol>
              </CRow>

              <div className="d-flex gap-2">
                <CButton
                  color="primary"
                  onClick={generateReport}
                  disabled={generating || !reportConfig.startDate || !reportConfig.endDate}
                >
                  {generating ? (
                    <>
                      <CSpinner size="sm" className="me-2" component="span" />
                      Generating...
                    </>
                  ) : (
                    <>
                      <CIcon icon={cilCloudDownload} className="me-2" />
                      Generate Report
                    </>
                  )}
                </CButton>
              </div>
            </CForm>

            {reportData && (
              <CAlert color="success">
                <strong>Report generated successfully!</strong> You can now download the report in different formats.
              </CAlert>
            )}
          </CCardBody>
        </CCard>
      </CCol>

      {/* Preview Modal */}
      <CModal visible={showPreview} onClose={() => setShowPreview(false)} size="lg" scrollable>
        <CModalHeader closeButton>
          <span>Report Preview</span>
        </CModalHeader>
        {reportData && (
          <CModalBody>
            <h5>{reportData.config.title}</h5>
            <p><strong>Organization:</strong> {reportData.config.organization || 'N/A'}</p>
            <p><strong>Generated:</strong> {new Date(reportData.generatedAt).toLocaleString()}</p>

            <h6 className="mt-4">Risk Assessment</h6>
            <p>
              <strong>Overall Risk:</strong>{' '}
              <span className={reportData.riskAssessment.overallRisk === 'HIGH' ? 'text-danger' : reportData.riskAssessment.overallRisk === 'MEDIUM' ? 'text-warning' : 'text-success'}>
                {reportData.riskAssessment.overallRisk}
              </span>
            </p>
            <p><strong>Risk Score:</strong> {reportData.riskAssessment.riskScore}/100</p>

            <h6 className="mt-3">Metrics Snapshot</h6>
            <CRow>
              <CCol md={6}>
                <p><strong>Total Experiments:</strong> {reportData.metrics.totalExperiments}</p>
                <p><strong>Success Rate:</strong> {reportData.metrics.successRate}%</p>
              </CCol>
              <CCol md={6}>
                <p><strong>Avg Execution Time:</strong> {reportData.metrics.avgExecutionDuration}ms</p>
                <p><strong>Rollback Success:</strong> {reportData.metrics.rollbackSuccessRate}%</p>
              </CCol>
            </CRow>
          </CModalBody>
        )}
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowPreview(false)}>
            Close
          </CButton>
          <CButton color="primary" onClick={downloadReport}>
            <CIcon icon={cilCloudDownload} className="me-2" />
            Download HTML
          </CButton>
          <CButton color="success" onClick={downloadCSV}>
            <CIcon icon={cilCloudDownload} className="me-2" />
            Download CSV
          </CButton>
        </CModalFooter>
      </CModal>
    </CRow>
  );
};

export default ComplianceReportGenerator;
