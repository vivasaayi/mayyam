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
