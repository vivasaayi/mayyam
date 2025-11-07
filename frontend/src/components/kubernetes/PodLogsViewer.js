import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  CAlert,
  CBadge,
  CButton,
  CCard,
  CCardBody,
  CCardHeader,
  CCol,
  CFormInput,
  CFormLabel,
  CFormSelect,
  CFormSwitch,
  CInputGroup,
  CInputGroupText,
  CRow,
  CSpinner,
} from "@coreui/react";
import { getPodLogs } from "../../services/kubernetesApiService";

const REFRESH_INTERVAL_MS = 10000;

const PodLogsViewer = ({ clusterId, namespace, pod, onClose }) => {
  const [container, setContainer] = useState("");
  const [tailLines, setTailLines] = useState("500");
  const [previous, setPrevious] = useState(false);
  const [logs, setLogs] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [liveUpdates, setLiveUpdates] = useState(false);

  const containers = useMemo(() => {
    if (!pod?.containers) {
      return [];
    }
    return pod.containers.map((c) => c.name).filter(Boolean);
  }, [pod]);

  useEffect(() => {
    if (containers.length > 0 && !container) {
      setContainer(containers[0]);
    }
  }, [container, containers]);

  const fetchLogs = useCallback(async () => {
    if (!clusterId || !pod) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await getPodLogs(clusterId, namespace, pod.name, {
        container: container || undefined,
        previous,
        tailLines: tailLines ? Number.parseInt(tailLines, 10) : undefined,
      });
      setLogs(data || "");
    } catch (err) {
      console.error("Failed to fetch pod logs", err);
      setError(err.message || "Unable to fetch logs");
      setLogs("");
    } finally {
      setLoading(false);
    }
  }, [clusterId, namespace, pod, container, previous, tailLines]);

  useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  useEffect(() => {
    if (!liveUpdates) {
      return undefined;
    }
    const timer = setInterval(fetchLogs, REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [fetchLogs, liveUpdates]);

  if (!pod) {
    return <CAlert color="info">Select a pod to inspect logs.</CAlert>;
  }

  return (
    <CCard className="h-100">
      <CCardHeader className="d-flex justify-content-between align-items-center">
        <div>
          <strong>Logs:</strong> {pod.name}
          <span className="ms-2">
            <CBadge color="secondary">{pod.namespace}</CBadge>
          </span>
        </div>
        <div className="d-flex align-items-center gap-2">
          <CFormSwitch
            id="logs-live-toggle"
            label="Live"
            checked={liveUpdates}
            onChange={(e) => setLiveUpdates(e.target.checked)}
          />
          <CButton
            size="sm"
            color="primary"
            variant="outline"
            disabled={loading}
            onClick={fetchLogs}
          >
            Refresh
          </CButton>
          <CButton size="sm" color="secondary" variant="outline" onClick={onClose}>
            Close
          </CButton>
        </div>
      </CCardHeader>
      <CCardBody className="d-flex flex-column">
        <CRow className="g-3 mb-3">
          <CCol md={4}>
            <CFormLabel htmlFor="logs-container-select">Container</CFormLabel>
            <CFormSelect
              id="logs-container-select"
              value={container}
              onChange={(e) => setContainer(e.target.value)}
              disabled={containers.length === 0}
            >
              {containers.length === 0 && <option value="">No containers found</option>}
              {containers.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </CFormSelect>
          </CCol>
          <CCol md={4}>
            <CFormLabel htmlFor="logs-tail-lines">Tail Lines</CFormLabel>
            <CInputGroup>
              <CInputGroupText>#</CInputGroupText>
              <CFormInput
                id="logs-tail-lines"
                type="number"
                min="1"
                max="5000"
                value={tailLines}
                onChange={(e) => setTailLines(e.target.value)}
              />
            </CInputGroup>
          </CCol>
          <CCol md={4} className="d-flex align-items-end">
            <CFormSwitch
              id="logs-previous-toggle"
              label="Previous instance"
              checked={previous}
              onChange={(e) => setPrevious(e.target.checked)}
            />
          </CCol>
        </CRow>
        {loading && (
          <div className="d-flex align-items-center gap-2 mb-3">
            <CSpinner size="sm" />
            <span>Fetching logs…</span>
          </div>
        )}
        {error && <CAlert color="danger">{error}</CAlert>}
        <div className="flex-grow-1 border rounded bg-black text-white p-3 overflow-auto" style={{ fontFamily: "monospace", fontSize: "0.85rem", lineHeight: 1.4 }}>
          {logs ? logs.split("\n").map((line, index) => (
            <div key={index}>{line}</div>
          )) : <span className="text-muted">No logs to display.</span>}
        </div>
      </CCardBody>
    </CCard>
  );
};

export default PodLogsViewer;
