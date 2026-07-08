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

const formatEvidence = (evidence) => {
  if (!evidence || Object.keys(evidence).length === 0) {
    return "No evidence payload";
  }
  return JSON.stringify(evidence);
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
        {reporting.incident_review?.report_id && (
          <div className="text-medium-emphasis small mb-2">
            {reporting.incident_review.report_id}
          </div>
        )}
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

const PostureSummary = ({ report }) => {
  const posture = report.posture;
  if (!posture) {
    return null;
  }

  const rules = posture.rules || [];
  return (
    <CCard className="mb-3">
      <CCardHeader>
        {formatToken(report.pillar)} Posture
        <CBadge
          color={posture.status === "pass" ? "success" : "danger"}
          className="ms-2"
        >
          {formatToken(posture.status)}
        </CBadge>
      </CCardHeader>
      <CCardBody>
        <CRow className="g-3 mb-3">
          <CCol md={4}>
            <div className="text-medium-emphasis small">Rules</div>
            <div className="fw-semibold">
              {posture.rules_failed || 0} failed of {posture.rules_evaluated || 0}
            </div>
          </CCol>
          <CCol md={8}>
            <div className="text-medium-emphasis small">Affected Resources</div>
            <div className="fw-semibold">
              {(posture.affected_resources || []).join(", ") || "None"}
            </div>
          </CCol>
        </CRow>
        <CTable small responsive>
          <CTableHead>
            <CTableRow>
              <CTableHeaderCell>Rule</CTableHeaderCell>
              <CTableHeaderCell>Status</CTableHeaderCell>
              <CTableHeaderCell>Reason Codes</CTableHeaderCell>
              <CTableHeaderCell>Affected Resources</CTableHeaderCell>
              <CTableHeaderCell>Actions</CTableHeaderCell>
            </CTableRow>
          </CTableHead>
          <CTableBody>
            {rules.map((rule) => (
              <CTableRow key={rule.rule_id}>
                <CTableDataCell>
                  <code>{rule.rule_id}</code>
                </CTableDataCell>
                <CTableDataCell>
                  <CBadge color={rule.status === "pass" ? "success" : "danger"}>
                    {rule.status}
                  </CBadge>
                </CTableDataCell>
                <CTableDataCell>
                  {(rule.reason_codes || []).map((reasonCode) => (
                    <code className="d-block" key={`${rule.rule_id}-${reasonCode}`}>
                      {reasonCode}
                    </code>
                  ))}
                </CTableDataCell>
                <CTableDataCell>
                  {(rule.affected_resources || []).join(", ") || "None"}
                </CTableDataCell>
                <CTableDataCell>
                  {rule.suppression_supported && (
                    <CBadge color="secondary" className="me-1">
                      Suppress supported
                    </CBadge>
                  )}
                  {rule.assignment_supported && (
                    <CBadge color="info">Assign supported</CBadge>
                  )}
                </CTableDataCell>
              </CTableRow>
            ))}
          </CTableBody>
        </CTable>
      </CCardBody>
    </CCard>
  );
};

const TriageSummary = ({ report }) => {
  const triage = report.triage_context;
  if (!triage) {
    return null;
  }

  const guardrails = triage.guardrails || {};
  const freshness = triage.freshness || {};
  const pagination = triage.pagination || {};
  const feedback = triage.feedback_capture || {};
  const deterministicOnly = guardrails.no_llm_invocation;
  return (
    <CCard className="mb-3">
      <CCardHeader>
        {formatToken(report.pillar)}{" "}
        {deterministicOnly ? "Triage Context" : "AI Triage"}
        <CBadge color="info" className="ms-2">
          {formatToken(triage.generation_mode)}
        </CBadge>
      </CCardHeader>
      <CCardBody>
        <CRow className="g-3 mb-3">
          <CCol md={4}>
            <div className="text-medium-emphasis small">Context</div>
            <div className="fw-semibold">{triage.context_builder_id}</div>
            <div className="small">{triage.prompt_template_id}</div>
            {triage.api_path && <div className="small">{triage.api_path}</div>}
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">
              {deterministicOnly ? "Provider Routing (not invoked)" : "Routing"}
            </div>
            <div className="fw-semibold">
              {(triage.provider_routing || []).join(", ") || "None"}
            </div>
            <div className="small">{triage.max_prompt_tokens || 0} token budget</div>
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Guardrails</div>
            <div className="fw-semibold">
              {guardrails.read_only_mode ? "Read only" : "Mutation capable"}
            </div>
            <div className="small">
              {guardrails.no_llm_invocation
                ? "Deterministic context only"
                : "LLM invocation allowed"}
            </div>
            {triage.audit_id_prefix && (
              <div className="small">{triage.audit_id_prefix}</div>
            )}
          </CCol>
        </CRow>
        {(triage.runbook_copy_markdown || feedback.supported || pagination.default_limit) && (
          <CRow className="g-3 mb-3">
            <CCol md={4}>
              <div className="text-medium-emphasis small">Runbook Copy</div>
              <div className="small">{triage.runbook_copy_markdown}</div>
            </CCol>
            <CCol md={4}>
              <div className="text-medium-emphasis small">Workflow Controls</div>
              <div className="small">
                {(triage.export_formats || []).join(", ") || "No exports"}
              </div>
              <div className="small">
                {pagination.default_limit || 0} default /{" "}
                {pagination.max_limit || 0} max evidence rows
              </div>
            </CCol>
            <CCol md={4}>
              <div className="text-medium-emphasis small">Feedback</div>
              <div className="small">
                {feedback.supported ? "Feedback capture enabled" : "Feedback disabled"}
              </div>
              <div className="small">{feedback.feedback_event_type}</div>
            </CCol>
          </CRow>
        )}
        {(freshness.freshness_source || (triage.error_codes || []).length > 0) && (
          <CRow className="g-3 mb-3">
            <CCol md={6}>
              <div className="text-medium-emphasis small">Freshness</div>
              <div className="small">
                {freshness.stale_data_blocks_ai_summary
                  ? "Stale data blocks AI summary"
                  : "Fresh enough for AI summary"}
              </div>
              <div className="small">
                {freshness.stale_resources || 0} stale resource(s) ·{" "}
                {freshness.freshness_source}
              </div>
            </CCol>
            <CCol md={6}>
              <div className="text-medium-emphasis small">Error Codes</div>
              <div className="small">
                {(triage.error_codes || []).join(", ") || "None"}
              </div>
            </CCol>
          </CRow>
        )}
        <CRow className="g-3">
          <CCol md={4}>
            <div className="text-medium-emphasis small">Facts</div>
            {(triage.facts || []).map((fact, idx) => (
              <div className="small" key={`fact-${idx}`}>
                {fact}
              </div>
            ))}
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Hypotheses</div>
            {(triage.hypotheses || []).map((hypothesis, idx) => (
              <div className="small" key={`hypothesis-${idx}`}>
                {hypothesis}
              </div>
            ))}
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Missing Data</div>
            {(triage.missing_data_questions || []).map((question, idx) => (
              <div className="small" key={`question-${idx}`}>
                {question}
              </div>
            ))}
          </CCol>
        </CRow>
        {(triage.follow_up_questions || []).length > 0 && (
          <div className="mt-3">
            <div className="text-medium-emphasis small">Follow-up Questions</div>
            {(triage.follow_up_questions || []).map((question, idx) => (
              <div className="small" key={`follow-up-${idx}`}>
                {question}
              </div>
            ))}
          </div>
        )}
        {(triage.evidence_citations || []).length > 0 && (
          <div className="mt-3">
            <div className="text-medium-emphasis small">Evidence Citations</div>
            {(triage.evidence_citations || []).map((citation, idx) => (
              <div className="small" key={`citation-${idx}`}>
                {citation.reason_code} · {citation.resource_id} ·{" "}
                {formatToken(citation.severity)}
                <pre className="small mb-2">
                  {formatEvidence(citation.evidence)}
                </pre>
              </div>
            ))}
          </div>
        )}
      </CCardBody>
    </CCard>
  );
};

const SloPolicySummary = ({ report }) => {
  const policy = report.slo_policy_tracking;
  if (!policy || !policy.objective) {
    return null;
  }

  const objective = policy.objective;
  const statusColor =
    objective.status === "on_track"
      ? "success"
      : objective.status === "breached"
        ? "danger"
        : "warning";

  return (
    <CCard className="mb-3">
      <CCardHeader>
        {formatToken(report.pillar)} SLO Policy
        <CBadge color={statusColor} className="ms-2">
          {formatToken(objective.status)}
        </CBadge>
      </CCardHeader>
      <CCardBody>
        <CRow className="g-3 mb-3">
          <CCol md={3}>
            <div className="text-medium-emphasis small">Objective</div>
            <div className="fw-semibold">{objective.objective_id}</div>
            <div className="small">
              score {objective.current_score} / target {objective.target_score_min}
            </div>
          </CCol>
          <CCol md={3}>
            <div className="text-medium-emphasis small">Policy State</div>
            <div className="fw-semibold">{formatToken(objective.policy_state)}</div>
            <div className="small">
              {formatToken(objective.trend_direction)} trend
            </div>
          </CCol>
          <CCol md={3}>
            <div className="text-medium-emphasis small">Ownership</div>
            <div className="fw-semibold">
              {(objective.owner_filters || []).join(", ") || "Unassigned"}
            </div>
            <div className="small">
              {(objective.environment_filters || []).join(", ") || "No environment"}
            </div>
          </CCol>
          <CCol md={3}>
            <div className="text-medium-emphasis small">Notifications</div>
            <div className="fw-semibold">
              {(objective.notification_targets || []).join(", ") || "None"}
            </div>
            <div className="small">
              {(objective.application_filters || []).join(", ") || "No application"}
            </div>
          </CCol>
        </CRow>
        <div className="small">
          {(objective.status_history || []).map((event) => (
            <CBadge color="secondary" className="me-1" key={event}>
              {formatToken(event)}
            </CBadge>
          ))}
        </div>
      </CCardBody>
    </CCard>
  );
};

const ForecastSummary = ({ report }) => {
  const forecast = report.forecasting;
  if (!forecast) {
    return null;
  }

  const forecastBand = forecast.forecast_band || {};
  const expectedIndex =
    forecastBand.expected_security_exposure_index ??
    forecastBand.expected_performance_pressure_index ??
    forecastBand.expected_recovery_exposure_index ??
    forecastBand.expected_monthly_cost_index;
  const lowerIndex =
    forecastBand.lower_security_exposure_index ??
    forecastBand.lower_performance_pressure_index ??
    forecastBand.lower_recovery_exposure_index ??
    forecastBand.lower_monthly_cost_index;
  const upperIndex =
    forecastBand.upper_security_exposure_index ??
    forecastBand.upper_performance_pressure_index ??
    forecastBand.upper_recovery_exposure_index ??
    forecastBand.upper_monthly_cost_index;
  const capacityRisk =
    forecast.exposure_capacity_risk ||
    forecast.performance_capacity_risk ||
    forecast.recovery_capacity_risk ||
    forecast.capacity_risk;
  const riskColor =
    forecast.risk_level === "low"
      ? "success"
      : forecast.risk_level === "blocked" || forecast.risk_level === "high"
        ? "danger"
        : "warning";

  return (
    <CCard className="mb-3">
      <CCardHeader>
        {formatToken(report.pillar)} Forecast
        <CBadge color={riskColor} className="ms-2">
          {formatToken(forecast.risk_level)}
        </CBadge>
      </CCardHeader>
      <CCardBody>
        <CRow className="g-3 mb-3">
          <CCol md={3}>
            <div className="text-medium-emphasis small">Workflow</div>
            <div className="fw-semibold">{forecast.workflow_id}</div>
            <div className="small">
              {forecast.baseline_window_days}d baseline ·{" "}
              {forecast.forecast_horizon_days}d horizon
            </div>
          </CCol>
          <CCol md={3}>
            <div className="text-medium-emphasis small">Forecast Band</div>
            <div className="fw-semibold">expected {expectedIndex ?? "n/a"}</div>
            <div className="small">
              {lowerIndex ?? "n/a"}-{upperIndex ?? "n/a"} ·{" "}
              {forecast.confidence_level}% confidence
            </div>
          </CCol>
          <CCol md={3}>
            <div className="text-medium-emphasis small">Capacity Risk</div>
            <div className="fw-semibold">{formatToken(capacityRisk)}</div>
            <div className="small">
              {forecast.blocked_by_stale_data ? "Blocked by stale data" : "Fresh enough"}
            </div>
          </CCol>
          <CCol md={3}>
            <div className="text-medium-emphasis small">Backtesting</div>
            <div className="fw-semibold">
              {formatToken(forecast.backtesting_fixture_status)}
            </div>
            <div className="small">
              {(forecast.risk_drivers || []).length} risk driver(s)
            </div>
          </CCol>
        </CRow>
        <div className="small mb-2">{forecast.blast_radius_summary}</div>
        <div className="small">
          {(forecast.threshold_controls || []).map((control) => (
            <CBadge color="secondary" className="me-1" key={control}>
              {formatToken(control)}
            </CBadge>
          ))}
        </div>
      </CCardBody>
    </CCard>
  );
};

const AgenticInvestigationSummary = ({ report }) => {
  const investigation = report.agentic_investigation;
  if (!investigation) {
    return null;
  }

  const steps = investigation.steps || [];
  const approvalGates = investigation.approval_gates || [];
  const evidenceCitations = investigation.evidence_citations || [];

  return (
    <CCard className="mb-3">
      <CCardHeader>
        {formatToken(report.pillar)} Agentic Investigation
        <CBadge color="info" className="ms-2">
          {formatToken(investigation.default_tool_mode)}
        </CBadge>
      </CCardHeader>
      <CCardBody>
        <CRow className="g-3 mb-3">
          <CCol md={4}>
            <div className="text-medium-emphasis small">Workflow</div>
            <div className="fw-semibold">{investigation.workflow_id}</div>
            <div className="small">
              {investigation.replay_required ? "Replay required" : "Replay optional"}
            </div>
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Budget</div>
            <div className="fw-semibold">
              {investigation.max_tool_calls || 0} max tool call(s)
            </div>
            <div className="small">
              {investigation.max_evidence_citations || 0} evidence citation(s)
            </div>
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Approval Gates</div>
            <div className="fw-semibold">{approvalGates.length} gate(s)</div>
            <div className="small">
              {approvalGates.length > 0
                ? "Mutation planning requires approval"
                : "Read-only diagnostics only"}
            </div>
          </CCol>
        </CRow>
        <CTable small responsive>
          <CTableHead>
            <CTableRow>
              <CTableHeaderCell>Step</CTableHeaderCell>
              <CTableHeaderCell>Mode</CTableHeaderCell>
              <CTableHeaderCell>Tool</CTableHeaderCell>
              <CTableHeaderCell>Resource</CTableHeaderCell>
              <CTableHeaderCell>Stop Condition</CTableHeaderCell>
            </CTableRow>
          </CTableHead>
          <CTableBody>
            {steps.map((step) => (
              <CTableRow key={step.step_id}>
                <CTableDataCell>
                  <code>{step.step_id}</code>
                  <div className="small">{formatToken(step.kind)}</div>
                </CTableDataCell>
                <CTableDataCell>
                  <CBadge
                    color={
                      step.tool_mode === "approval_required" ? "warning" : "success"
                    }
                  >
                    {formatToken(step.tool_mode)}
                  </CBadge>
                </CTableDataCell>
                <CTableDataCell>
                  <code>{step.tool_name}</code>
                  <div className="small">{step.reason_code}</div>
                </CTableDataCell>
                <CTableDataCell>{step.target_resource_id}</CTableDataCell>
                <CTableDataCell>{step.stop_condition}</CTableDataCell>
              </CTableRow>
            ))}
          </CTableBody>
        </CTable>
        {approvalGates.length > 0 && (
          <div className="mt-3">
            <div className="text-medium-emphasis small">Approval Gates</div>
            {approvalGates.map((gate) => (
              <div className="small mb-2" key={gate.gate_id}>
                <code>{gate.gate_id}</code> · {gate.target_resource_id} ·{" "}
                {gate.required_approval}
                <div>{gate.blast_radius}</div>
                <div>
                  Rollback note{" "}
                  {gate.rollback_note_required ? "required" : "not required"}
                </div>
              </div>
            ))}
          </div>
        )}
        {evidenceCitations.length > 0 && (
          <div className="mt-3">
            <div className="text-medium-emphasis small">
              Investigation Evidence
            </div>
            {evidenceCitations.map((citation, idx) => (
              <div className="small" key={`investigation-citation-${idx}`}>
                {citation.reason_code} · {citation.resource_id}
              </div>
            ))}
          </div>
        )}
      </CCardBody>
    </CCard>
  );
};

const RemediationWorkflowSummary = ({ report }) => {
  const remediation = report.remediation_workflow;
  if (!remediation) {
    return null;
  }

  const actions = remediation.actions || [];
  const approvalGates = remediation.approval_gates || [];

  return (
    <CCard className="mb-3">
      <CCardHeader>
        {formatToken(report.pillar)} Remediation
        <CBadge
          color={remediation.stale_data_blocks_execution ? "danger" : "warning"}
          className="ms-2"
        >
          {remediation.read_only_mode ? "Dry Run" : "Execution Enabled"}
        </CBadge>
      </CCardHeader>
      <CCardBody>
        <CRow className="g-3 mb-3">
          <CCol md={4}>
            <div className="text-medium-emphasis small">Workflow</div>
            <div className="fw-semibold">{remediation.workflow_id}</div>
            <div className="small">{remediation.audit_stream}</div>
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Approval</div>
            <div className="fw-semibold">{remediation.rbac_permission}</div>
            <div className="small">{approvalGates.length} gate(s)</div>
          </CCol>
          <CCol md={4}>
            <div className="text-medium-emphasis small">Execution Guard</div>
            <div className="fw-semibold">
              {remediation.stale_data_blocks_execution
                ? "Blocked by stale data"
                : "Pending approval"}
            </div>
            <div className="small">{actions.length} dry-run action(s)</div>
          </CCol>
        </CRow>
        <CTable small responsive>
          <CTableHead>
            <CTableRow>
              <CTableHeaderCell>Action</CTableHeaderCell>
              <CTableHeaderCell>Status</CTableHeaderCell>
              <CTableHeaderCell>Resource</CTableHeaderCell>
              <CTableHeaderCell>Approval</CTableHeaderCell>
              <CTableHeaderCell>Rollback</CTableHeaderCell>
            </CTableRow>
          </CTableHead>
          <CTableBody>
            {actions.map((action) => (
              <CTableRow key={action.action_id}>
                <CTableDataCell>
                  <code>{action.action_id}</code>
                  <div className="small">{formatToken(action.kind)}</div>
                  <div className="small">{action.audit_event_type}</div>
                </CTableDataCell>
                <CTableDataCell>
                  <CBadge
                    color={
                      action.status === "blocked_missing_evidence"
                        ? "danger"
                        : "warning"
                    }
                  >
                    {formatToken(action.status)}
                  </CBadge>
                  <div className="small">{action.dry_run ? "Dry run" : "Executable"}</div>
                </CTableDataCell>
                <CTableDataCell>{action.target_resource_id}</CTableDataCell>
                <CTableDataCell>
                  {action.requires_approval ? "Required" : "Not required"}
                  <div className="small">{action.approval_gate_id}</div>
                </CTableDataCell>
                <CTableDataCell>{action.rollback_note}</CTableDataCell>
              </CTableRow>
            ))}
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
      {data.reports.map((report) => (
        <PostureSummary key={`${report.pillar}-posture`} report={report} />
      ))}
      {data.reports.map((report) => (
        <SloPolicySummary key={`${report.pillar}-slo-policy`} report={report} />
      ))}
      {data.reports.map((report) => (
        <ForecastSummary key={`${report.pillar}-forecast`} report={report} />
      ))}
      {data.reports.map((report) => (
        <TriageSummary key={`${report.pillar}-triage`} report={report} />
      ))}
      {data.reports.map((report) => (
        <AgenticInvestigationSummary
          key={`${report.pillar}-agentic-investigation`}
          report={report}
        />
      ))}
      {data.reports.map((report) => (
        <RemediationWorkflowSummary
          key={`${report.pillar}-remediation`}
          report={report}
        />
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
