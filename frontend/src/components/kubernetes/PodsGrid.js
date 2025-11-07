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
import { getPods } from "../../services/kubernetesApiService";

const REFRESH_INTERVAL_MS = 15000;

const PodsGrid = ({ clusterId, namespace, onSelectPod, onViewEvents }) => {
  const [pods, setPods] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [liveUpdates, setLiveUpdates] = useState(false);

  const effectiveNamespace = useMemo(() => {
    if (!namespace || namespace === "") {
      return "all";
    }
    return namespace;
  }, [namespace]);

  const fetchPods = useCallback(async () => {
    if (!clusterId) {
      setPods([]);
      return;
    }
    try {
      setLoading(true);
      setError(null);
      const data = await getPods(clusterId, effectiveNamespace);
      setPods(Array.isArray(data) ? data : []);
    } catch (err) {
      console.error("Failed to fetch pods", err);
      setError(err.message || "Unable to load pods");
      setPods([]);
    } finally {
      setLoading(false);
    }
  }, [clusterId, effectiveNamespace]);

  useEffect(() => {
    fetchPods();
  }, [fetchPods]);

  useEffect(() => {
    if (!liveUpdates || !clusterId) {
      return undefined;
    }
    const timer = setInterval(fetchPods, REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [clusterId, fetchPods, liveUpdates]);

  if (!clusterId) {
    return <CAlert color="info">Select a cluster to view pods.</CAlert>;
  }

  return (
    <CCard>
      <CCardHeader className="d-flex justify-content-between align-items-center">
        <strong>Pods</strong>
        <div className="d-flex align-items-center gap-3">
          <div className="d-flex align-items-center gap-2">
            <CFormSwitch
              id="pods-live-toggle"
              label="Live refresh"
              checked={liveUpdates}
              onChange={(e) => setLiveUpdates(e.target.checked)}
            />
          </div>
          <CButton
            size="sm"
            color="primary"
            variant="outline"
            disabled={loading}
            onClick={fetchPods}
          >
            Refresh
          </CButton>
        </div>
      </CCardHeader>
      <CCardBody>
        {loading && (
          <div className="d-flex align-items-center gap-2">
            <CSpinner size="sm" />
            <span>Loading pods…</span>
          </div>
        )}
        {error && <CAlert color="danger">{error}</CAlert>}
        {!loading && !error && pods.length === 0 && (
          <CAlert color="secondary">No pods found for the selected scope.</CAlert>
        )}
        {!loading && !error && pods.length > 0 && (
          <CTable responsive hover align="middle" className="mb-0">
            <CTableHead color="light">
              <CTableRow>
                <CTableHeaderCell scope="col">Name</CTableHeaderCell>
                <CTableHeaderCell scope="col">Namespace</CTableHeaderCell>
                <CTableHeaderCell scope="col">Status</CTableHeaderCell>
                <CTableHeaderCell scope="col">Node</CTableHeaderCell>
                <CTableHeaderCell scope="col">Age</CTableHeaderCell>
                <CTableHeaderCell scope="col">Restarts</CTableHeaderCell>
                <CTableHeaderCell scope="col">Containers</CTableHeaderCell>
                <CTableHeaderCell scope="col" className="text-end">
                  Actions
                </CTableHeaderCell>
              </CTableRow>
            </CTableHead>
            <CTableBody>
              {pods.map((pod) => (
                <CTableRow key={`${pod.namespace}/${pod.name}`}>
                  <CTableDataCell>{pod.name}</CTableDataCell>
                  <CTableDataCell>{pod.namespace || "(cluster)"}</CTableDataCell>
                  <CTableDataCell>
                    <StatusBadge status={pod.status} />
                  </CTableDataCell>
                  <CTableDataCell>{pod.node_name || "—"}</CTableDataCell>
                  <CTableDataCell>{pod.age}</CTableDataCell>
                  <CTableDataCell>{pod.restart_count}</CTableDataCell>
                  <CTableDataCell>
                    {pod.containers
                      ?.map((c) => `${c.name}${c.ready ? " ✅" : ""}`)
                      .join(", ") || "—"}
                  </CTableDataCell>
                  <CTableDataCell className="text-end">
                    <CRow className="g-2">
                      <CCol xs="auto">
                        <CButton
                          size="sm"
                          color="info"
                          variant="outline"
                          onClick={() => onSelectPod && onSelectPod(pod)}
                        >
                          Logs
                        </CButton>
                      </CCol>
                      <CCol xs="auto">
                        <CButton
                          size="sm"
                          color="warning"
                          variant="outline"
                          onClick={() => onViewEvents && onViewEvents(pod)}
                        >
                          Events
                        </CButton>
                      </CCol>
                    </CRow>
                  </CTableDataCell>
                </CTableRow>
              ))}
            </CTableBody>
          </CTable>
        )}
      </CCardBody>
    </CCard>
  );
};

const StatusBadge = ({ status }) => {
  if (!status) {
    return <CBadge color="secondary">unknown</CBadge>;
  }
  const normalized = status.toLowerCase();
  const variant =
    normalized === "running"
      ? "success"
      : normalized === "pending"
      ? "warning"
      : normalized === "failed"
      ? "danger"
      : "info";
  return <CBadge color={variant}>{status}</CBadge>;
};

export default PodsGrid;
