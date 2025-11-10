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


import React, { useCallback, useEffect, useState } from "react";
import {
  CAlert,
  CBadge,
  CButton,
  CCard,
  CCardBody,
  CCardHeader,
  CListGroup,
  CListGroupItem,
  CSpinner,
} from "@coreui/react";
import { getPodEvents } from "../../services/kubernetesApiService";

const REFRESH_INTERVAL_MS = 15000;

const EventsStream = ({ clusterId, pod, onClose }) => {
  const [events, setEvents] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [liveUpdates, setLiveUpdates] = useState(true);

  const fetchEvents = useCallback(async () => {
    if (!clusterId || !pod) {
      setEvents([]);
      return;
    }
    try {
      setLoading(true);
      setError(null);
      const data = await getPodEvents(clusterId, pod.namespace, pod.name);
      setEvents(Array.isArray(data) ? data : []);
    } catch (err) {
      console.error("Failed to fetch pod events", err);
      setError(err.message || "Unable to fetch events");
      setEvents([]);
    } finally {
      setLoading(false);
    }
  }, [clusterId, pod]);

  useEffect(() => {
    fetchEvents();
  }, [fetchEvents]);

  useEffect(() => {
    if (!liveUpdates || !clusterId || !pod) {
      return undefined;
    }
    const timer = setInterval(fetchEvents, REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [clusterId, fetchEvents, liveUpdates, pod]);

  if (!pod) {
    return <CAlert color="info">Select a pod to view related events.</CAlert>;
  }

  return (
    <CCard className="h-100">
      <CCardHeader className="d-flex justify-content-between align-items-center">
        <div>
          <strong>Events:</strong> {pod.name}
          <span className="ms-2">
            <CBadge color="secondary">{pod.namespace}</CBadge>
          </span>
        </div>
        <div className="d-flex align-items-center gap-2">
          <label className="small mb-0 d-flex align-items-center gap-1">
            <input
              type="checkbox"
              checked={liveUpdates}
              onChange={(e) => setLiveUpdates(e.target.checked)}
            />
            Live
          </label>
          <CButton size="sm" color="primary" variant="outline" disabled={loading} onClick={fetchEvents}>
            Refresh
          </CButton>
          <CButton size="sm" color="secondary" variant="outline" onClick={onClose}>
            Close
          </CButton>
        </div>
      </CCardHeader>
      <CCardBody>
        {loading && (
          <div className="d-flex align-items-center gap-2 mb-3">
            <CSpinner size="sm" />
            <span>Fetching events…</span>
          </div>
        )}
        {error && <CAlert color="danger">{error}</CAlert>}
        {!loading && !error && events.length === 0 && (
          <CAlert color="secondary">No events recorded for this pod.</CAlert>
        )}
        {!loading && !error && events.length > 0 && (
          <CListGroup>
            {events.map((event, index) => {
              const timestamp =
                event.lastTimestamp ||
                event.last_timestamp ||
                event.eventTime ||
                event.event_time ||
                event.firstTimestamp ||
                event.first_timestamp ||
                "Unknown time";
              return (
                <CListGroupItem key={index}>
                  <div className="d-flex justify-content-between align-items-start">
                    <div>
                      <div className="fw-semibold">{event.reason || "Event"}</div>
                      <div className="text-muted small">{event.type}</div>
                      <div className="mt-2" style={{ whiteSpace: "pre-line" }}>
                        {event.message || "No event message provided."}
                      </div>
                    </div>
                    <div className="text-end small text-muted">{timestamp}</div>
                  </div>
                </CListGroupItem>
              );
            })}
          </CListGroup>
        )}
      </CCardBody>
    </CCard>
  );
};

export default EventsStream;
