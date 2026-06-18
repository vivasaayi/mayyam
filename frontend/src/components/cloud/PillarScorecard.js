import React, { useState } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CCol,
  CRow,
  CBadge,
  CTable,
  CTableBody,
  CTableDataCell,
  CTableHead,
  CTableHeaderCell,
  CTableRow,
  CCollapse,
  CButton,
} from "@coreui/react";

const SEVERITY_COLOR = { high: "danger", medium: "warning", low: "info" };

const formatToken = (value) =>
  String(value || "")
    .replace(/_/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase());

const scoreColor = (score) => {
  if (score >= 90) return "success";
  if (score >= 70) return "warning";
  return "danger";
};

const FindingRow = ({ finding }) => {
  const [open, setOpen] = useState(false);
  return (
    <>
      <CTableRow>
        <CTableDataCell>
          <CBadge color={SEVERITY_COLOR[finding.severity] || "secondary"}>
            {finding.severity}
          </CBadge>
        </CTableDataCell>
        <CTableDataCell>
          <code>{finding.reason_code}</code>
        </CTableDataCell>
        <CTableDataCell>{finding.resource_id}</CTableDataCell>
        <CTableDataCell>{finding.message}</CTableDataCell>
        <CTableDataCell>
          <CButton
            color="link"
            size="sm"
            onClick={() => setOpen(!open)}
            aria-expanded={open}
          >
            {open ? "Hide" : "Evidence"}
          </CButton>
        </CTableDataCell>
      </CTableRow>
      {open && (
        <CTableRow>
          <CTableDataCell colSpan={5}>
            <CCollapse visible={open}>
              <pre className="mb-0 small bg-light p-2">
                {JSON.stringify(finding.evidence, null, 2)}
              </pre>
            </CCollapse>
          </CTableDataCell>
        </CTableRow>
      )}
    </>
  );
};

const ReportingSummary = ({ report }) => {
  const reporting = report.reporting;
  if (!reporting) {
    return null;
  }

  const incidentRows = reporting.incident_review?.rows || [];
  return (
    <CCard className="mb-3">
      <CCardHeader>
        {formatToken(report.pillar)} Reporting
        <CBadge
          color={reporting.stale_data_blocks_delivery ? "danger" : "success"}
          className="ms-2"
        >
          {formatToken(reporting.scheduled_delivery_state)}
        </CBadge>
      </CCardHeader>
      <CCardBody>
        <CRow className="g-3 mb-3">
          <CCol md={4}>
            <div className="text-medium-emphasis small">Executive Report</div>
            <div className="fw-semibold">
              {reporting.executive_summary?.report_id}
            </div>
            <div className="small">
              {reporting.executive_summary?.rules_failed || 0} failed rule(s) ·{" "}
              {reporting.executive_summary?.affected_resources?.length || 0} affected
            </div>
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Blast Radius</div>
            <div className="fw-semibold">
              {formatToken(reporting.executive_summary?.blast_radius_summary)}
            </div>
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Evidence Gaps</div>
            <div className="fw-semibold">
              {reporting.missing_data_reason_codes?.length || 0} missing signal(s)
            </div>
            <div className="small">
              {reporting.stale_data_blocks_delivery
                ? "Fresh resilience evidence required"
                : "Ready for review"}
            </div>
          </CCol>
        </CRow>
        <CTable small responsive>
          <CTableHead>
            <CTableRow>
              <CTableHeaderCell>Reason Code</CTableHeaderCell>
              <CTableHeaderCell>Resource</CTableHeaderCell>
              <CTableHeaderCell>Recovery Note</CTableHeaderCell>
              <CTableHeaderCell>Suppression</CTableHeaderCell>
            </CTableRow>
          </CTableHead>
          <CTableBody>
            {incidentRows.map((row, idx) => (
              <CTableRow key={`${row.reason_code}-${row.resource_id}-${idx}`}>
                <CTableDataCell>
                  <code>{row.reason_code}</code>
                </CTableDataCell>
                <CTableDataCell>{row.resource_id}</CTableDataCell>
                <CTableDataCell>{row.recovery_note}</CTableDataCell>
                <CTableDataCell>
                  {row.suppression_supported ? "Supported" : "Not supported"}
                </CTableDataCell>
              </CTableRow>
            ))}
            {incidentRows.length === 0 && (
              <CTableRow>
                <CTableDataCell colSpan={4} className="text-center text-success">
                  No incident-review rows for this reporting bundle.
                </CTableDataCell>
              </CTableRow>
            )}
          </CTableBody>
        </CTable>
      </CCardBody>
    </CCard>
  );
};

// Renders the deterministic pillar reports returned by
// /api/aws/inventory/<service>/pillars: one score card per pillar plus a
// reason-coded findings table with raw evidence.
const PillarScorecard = ({ data }) => {
  if (!data || !Array.isArray(data.reports)) {
    return null;
  }
  return (
    <>
      <CRow className="mb-3">
        {data.reports.map((report) => (
          <CCol sm={4} key={report.pillar}>
            <CCard color={scoreColor(report.score)} textColor="white">
              <CCardBody>
                <div className="fs-6 text-uppercase">{report.pillar}</div>
                <div className="fs-2 fw-bold">{report.score}</div>
                <div className="small">
                  {report.findings.length} finding(s) ·{" "}
                  {report.resources_evaluated} resource(s)
                  {report.stale_resources > 0 &&
                    ` · ${report.stale_resources} stale`}
                </div>
              </CCardBody>
            </CCard>
          </CCol>
        ))}
      </CRow>
      {data.reports.map((report) => (
        <ReportingSummary key={`${report.pillar}-reporting`} report={report} />
      ))}
      <CCard>
        <CCardHeader>
          Findings
          <span className="text-medium-emphasis small ms-2">
            evaluated {data.evaluated_at} · stale after {data.stale_after_hours}h
          </span>
        </CCardHeader>
        <CCardBody>
          <CTable small responsive>
            <CTableHead>
              <CTableRow>
                <CTableHeaderCell>Severity</CTableHeaderCell>
                <CTableHeaderCell>Reason Code</CTableHeaderCell>
                <CTableHeaderCell>Resource</CTableHeaderCell>
                <CTableHeaderCell>Message</CTableHeaderCell>
                <CTableHeaderCell />
              </CTableRow>
            </CTableHead>
            <CTableBody>
              {data.reports.flatMap((report) =>
                report.findings.map((finding, idx) => (
                  <FindingRow
                    key={`${report.pillar}-${finding.reason_code}-${finding.resource_id}-${idx}`}
                    finding={finding}
                  />
                ))
              )}
              {data.reports.every((r) => r.findings.length === 0) && (
                <CTableRow>
                  <CTableDataCell colSpan={5} className="text-center text-success">
                    No findings — all evaluated resources pass deterministic checks.
                  </CTableDataCell>
                </CTableRow>
              )}
            </CTableBody>
          </CTable>
        </CCardBody>
      </CCard>
    </>
  );
};

export default PillarScorecard;
