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

import React, { useState } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CSpinner,
  CAlert,
  CFormSelect,
  CFormLabel,
  CBadge
} from "@coreui/react";
import ReactMarkdown from "react-markdown";
import api from "../../services/api";

const MySqlTriage = ({ connection }) => {
  const [triageType, setTriageType] = useState("performance");
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState(null);
  const [error, setError] = useState(null);

  const runTriage = async (type = triageType) => {
    try {
      setLoading(true);
      setError(null);
      setResult(null);
      
      const response = await api.get(`/api/ai/analyze/mysql/${connection.id}/${type}`);
      setResult(response.data);
    } catch (err) {
      setError("Triage failed: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const triageOptions = [
    { id: "performance", name: "Performance Root Cause", icon: "🚀" },
    { id: "connection", name: "Connection & Thread Triage", icon: "🔗" },
    { id: "index", name: "Index Advisor", icon: "📇" }
  ];

  return (
    <div className="mysql-triage">
      <CRow className="mb-4">
        <CCol lg={12}>
          <CCard className="border-primary shadow-sm">
            <CCardHeader className="bg-primary text-white d-flex justify-content-between align-items-center">
              <span><strong>🤖 AI Database Triage</strong></span>
              <CBadge color="light" className="text-primary">MySQL Expert Mode</CBadge>
            </CCardHeader>
            <CCardBody>
              <CRow className="align-items-end">
                <CCol md={6}>
                  <CFormLabel htmlFor="triageType">Select Triage Workflow</CFormLabel>
                  <CFormSelect 
                    id="triageType" 
                    value={triageType}
                    onChange={(e) => setTriageType(e.target.value)}
                    disabled={loading}
                  >
                    {triageOptions.map(opt => (
                      <option key={opt.id} value={opt.id}>
                        {opt.icon} {opt.name}
                      </option>
                    ))}
                  </CFormSelect>
                </CCol>
                <CCol md={6}>
                  <CButton 
                    color="primary" 
                    className="w-100" 
                    onClick={() => runTriage()}
                    disabled={loading}
                  >
                    {loading ? (
                      <><CSpinner size="sm" className="me-2"/> Running Analysis...</>
                    ) : (
                      "🚀 Run AI Triage"
                    )}
                  </CButton>
                </CCol>
              </CRow>
              <div className="mt-3 text-muted small">
                AI will analyze real-time metrics, slow query logs, and system variables to provide actionable insights.
              </div>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      {error && (
        <CAlert color="danger" className="mb-4">
          {error}
        </CAlert>
      )}

      {loading && (
        <div className="text-center my-5">
          <CSpinner color="primary" variant="grow" />
          <div className="mt-2 text-primary fw-semibold">Consulting AI DBA Expert...</div>
        </div>
      )}

      {result && (
        <CRow>
          <CCol lg={8}>
            <CCard className="mb-4 shadow-sm border-0">
              <CCardHeader className="bg-light border-bottom">
                <strong>Analysis Results</strong>
              </CCardHeader>
              <CCardBody className="p-4 bg-white rounded-bottom">
                <div className="markdown-content triage-report">
                  <ReactMarkdown>{result.content}</ReactMarkdown>
                </div>
              </CCardBody>
            </CCard>
          </CCol>
          <CCol lg={4}>
            <CCard className="mb-4 shadow-sm">
              <CCardHeader className="bg-dark text-white">
                <strong>💡 Suggestions & Follow-ups</strong>
              </CCardHeader>
              <CCardBody>
                <div className="d-grid gap-2">
                  {result.related_questions?.map((q, idx) => (
                    <CButton 
                      key={idx} 
                      color="light" 
                      variant="outline" 
                      className="text-start border-1 shadow-sm"
                      onClick={() => {
                        // In a real app, this might open a chat or run another triage
                        alert(`Feature Coming Soon: ${q}`);
                      }}
                    >
                      {q}
                    </CButton>
                  ))}
                </div>
                <hr />
                <div className="small text-muted">
                  <strong>Resource:</strong> {connection.name}<br />
                  <strong>Host:</strong> {connection.host}
                </div>
              </CCardBody>
            </CCard>
          </CCol>
        </CRow>
      )}
    </div>
  );
};

export default MySqlTriage;
