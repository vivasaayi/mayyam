import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  CAlert,
  CBadge,
  CButton,
  CCard,
  CCardBody,
  CCardHeader,
  CCol,
  CFormSwitch,
  CRow,
  CSpinner,
  CTable,
  CTableBody,
  CTableDataCell,
  CTableHead,
  CTableHeaderCell,
  CTableRow,
} from "@coreui/react";
import { getClusterMetrics } from "../../services/kubernetesApiService";

const REFRESH_INTERVAL_MS = 20000;

const ClusterMetricsPanel = ({ clusterId, namespace }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [metrics, setMetrics] = useState(null);
  const [liveUpdates, setLiveUpdates] = useState(false);

  const effectiveNamespace = useMemo(() => {
    if (!namespace || namespace === "") {
      return null;
    }
    if (namespace === "all") {
      return null;
    }
    return namespace;
  }, [namespace]);

  const fetchMetrics = useCallback(async () => {
    if (!clusterId) {
      setMetrics(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await getClusterMetrics(clusterId, effectiveNamespace);
      setMetrics(data);
    } catch (err) {
      console.error("Failed to load cluster metrics", err);
      setError(err.message || "Unable to load metrics");
      setMetrics(null);
    } finally {
      setLoading(false);
    }
  }, [clusterId, effectiveNamespace]);

  useEffect(() => {
    fetchMetrics();
  }, [fetchMetrics]);

  useEffect(() => {
    if (!liveUpdates || !clusterId) {
      return undefined;
    }
    const timer = setInterval(fetchMetrics, REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [clusterId, fetchMetrics, liveUpdates]);

  if (!clusterId) {
    return <CAlert color="info">Select a cluster to view metrics.</CAlert>;
  }

  return (
    <CCard>
      <CCardHeader className="d-flex justify-content-between align-items-center">
        <strong>Cluster Metrics</strong>
        <div className="d-flex align-items-center gap-3">
          <CFormSwitch
            id="metrics-live-toggle"
            label="Live refresh"
            checked={liveUpdates}
            onChange={(e) => setLiveUpdates(e.target.checked)}
          />
          <CButton
            size="sm"
            color="primary"
            variant="outline"
            disabled={loading}
            onClick={fetchMetrics}
          >
            Refresh
          </CButton>
        </div>
      </CCardHeader>
      <CCardBody>
        {loading && (
          <div className="d-flex align-items-center gap-2">
            <CSpinner size="sm" />
            <span>Loading metrics…</span>
          </div>
        )}
        {error && <CAlert color="danger">{error}</CAlert>}
        {!loading && !error && metrics && !metrics.metrics_available && (
          <CAlert color="warning">
            {metrics.message || "Metrics API is unavailable for this cluster."}
          </CAlert>
        )}
        {!loading && !error && metrics && metrics.metrics_available && (
          <>
            <CRow className="mb-4">
              <CCol md={6}>
                <UsageSummaryCard
                  title="Nodes"
                  summary={metrics.node_totals}
                  emoji="🖥️"
                />
              </CCol>
              <CCol md={6}>
                <UsageSummaryCard
                  title="Pods"
                  summary={metrics.pod_totals}
                  emoji="📦"
                />
              </CCol>
            </CRow>
            <CRow>
              <CCol md={6} className="mb-4">
                <h6 className="mb-3">Node Breakdown</h6>
                <MetricsTable
                  headers={["Node", "CPU", "Memory"]}
                  rows={(metrics.nodes || []).map((node) => [
                    node.name,
                    node.cpu_formatted,
                    node.memory_formatted,
                  ])}
                  emptyMessage="No node metrics available"
                />
              </CCol>
              <CCol md={6} className="mb-4">
                <h6 className="mb-3">Top Pods by CPU</h6>
                <MetricsTable
                  headers={["Pod", "Namespace", "CPU", "Memory"]}
                  rows={(metrics.pods || []).map((pod) => [
                    pod.name,
                    <NamespaceBadge key={`${pod.namespace}-${pod.name}`} value={pod.namespace} />,
                    pod.cpu_formatted,
                    pod.memory_formatted,
                  ])}
                  emptyMessage="No pod metrics available"
                />
              </CCol>
            </CRow>
          </>
        )}
      </CCardBody>
    </CCard>
  );
};

const UsageSummaryCard = ({ title, summary, emoji }) => (
  <CCard className="h-100 border-0 shadow-sm">
    <CCardBody>
      <div className="d-flex justify-content-between align-items-center mb-2">
        <h6 className="mb-0">{title}</h6>
        <span role="img" aria-label={title} style={{ fontSize: "1.5rem" }}>
          {emoji}
        </span>
      </div>
      <div className="small text-uppercase text-muted">Resources</div>
      <div className="fs-4 fw-semibold">{summary?.count || 0}</div>
      <div className="mt-3">
        <div className="text-muted small">CPU</div>
        <div className="fw-semibold">{summary?.cpu_formatted || "0"}</div>
      </div>
      <div className="mt-3">
        <div className="text-muted small">Memory</div>
        <div className="fw-semibold">{summary?.memory_formatted || "0"}</div>
      </div>
    </CCardBody>
  </CCard>
);

const MetricsTable = ({ headers, rows, emptyMessage }) => {
  if (!rows || rows.length === 0) {
    return <CAlert color="secondary">{emptyMessage}</CAlert>;
  }
  return (
    <CTable small responsive hover align="middle">
      <CTableHead color="light">
        <CTableRow>
          {headers.map((header) => (
            <CTableHeaderCell key={header}>{header}</CTableHeaderCell>
          ))}
        </CTableRow>
      </CTableHead>
      <CTableBody>
        {rows.map((row, rowIndex) => (
          <CTableRow key={rowIndex}>
            {row.map((cell, cellIndex) => (
              <CTableDataCell key={`${rowIndex}-${cellIndex}`}>
                {cell}
              </CTableDataCell>
            ))}
          </CTableRow>
        ))}
      </CTableBody>
    </CTable>
  );
};

const NamespaceBadge = ({ value }) => {
  const label = value || "default";
  const variant = label === "default" ? "info" : "secondary";
  return <CBadge color={variant}>{label}</CBadge>;
};

export default ClusterMetricsPanel;
