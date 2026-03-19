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


import React, { useState, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CSpinner,
  CBadge,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CAlert,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane
} from "@coreui/react";
import { CChart } from "@coreui/react-chartjs";
import PageHeader from "../components/layout/PageHeader";
import { getQueryFingerprint, getFingerprintAnalysis, generateAiAnalysis } from "../services/api";

const QueryFingerprintDetail = () => {
  const { id } = useParams();
  const navigate = useNavigate();
  const [fingerprint, setFingerprint] = useState(null);
  const [analysis, setAnalysis] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [activeTab, setActiveTab] = useState("overview");
  const [runningAi, setRunningAi] = useState(false);

  useEffect(() => {
    fetchData();
  }, [id]);

  const fetchData = async () => {
    setLoading(true);
    try {
      const [fp, an] = await Promise.all([
        getQueryFingerprint(id),
        getFingerprintAnalysis(id)
      ]);
      setFingerprint(fp);
      setAnalysis(an);
    } catch (err) {
      setError("Failed to fetch fingerprint details: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleRunAiAnalysis = async () => {
    setRunningAi(true);
    try {
      const res = await generateAiAnalysis({
        fingerprint_id: id,
        prompt_type: "query_optimization"
      });
      setAnalysis(prev => ({ ...prev, ai_insights: res.insights }));
    } catch (err) {
      setError("AI Analysis failed: " + err.message);
    } finally {
      setRunningAi(false);
    }
  };

  if (loading) return <div className="text-center mt-5"><CSpinner /></div>;

  return (
    <div>
      <PageHeader
        title="Query Fingerprint Detail"
        breadcrumbs={[
          { label: "Slow Queries", link: "/slow-queries" },
          { label: "Fingerprints", link: "/query-fingerprints" },
          { label: "Detail" }
        ]}
      />

      {error && <CAlert color="danger">{error}</CAlert>}

      <CCard className="mb-4 shadow-sm border-0">
        <CCardHeader className="bg-white border-bottom-0 pt-3">
          <div className="d-flex justify-content-between">
            <h5 className="mb-0">Fingerprint: <span className="text-muted font-monospace small">{id}</span></h5>
            <CButton color="primary" onClick={handleRunAiAnalysis} disabled={runningAi}>
              {runningAi ? <CSpinner size="sm" /> : "Run AI Analysis"}
            </CButton>
          </div>
        </CCardHeader>
        <CCardBody>
          <CNav variant="tabs">
            <CNavItem>
              <CNavLink active={activeTab === "overview"} onClick={() => setActiveTab("overview")}>Overview</CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink active={activeTab === "sql"} onClick={() => setActiveTab("sql")}>Normalized SQL</CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink active={activeTab === "explain"} onClick={() => setActiveTab("explain")}>EXPLAIN Plans</CNavLink>
            </CNavItem>
          </CNav>
          <CTabContent className="p-3 border border-top-0 rounded-bottom">
            <CTabPane visible={activeTab === "overview"}>
              <CRow>
                <CCol md={6}>
                  <CTable small borderless>
                    <CTableBody>
                      <CTableRow>
                        <CTableDataCell className="fw-bold">Execution Count:</CTableDataCell>
                        <CTableDataCell>{fingerprint.execution_count}</CTableDataCell>
                      </CTableRow>
                      <CTableRow>
                        <CTableDataCell className="fw-bold">Avg Latency:</CTableDataCell>
                        <CTableDataCell>{(fingerprint.avg_query_time).toFixed(2)}s</CTableDataCell>
                      </CTableRow>
                      <CTableRow>
                        <CTableDataCell className="fw-bold">Waste Score:</CTableDataCell>
                        <CTableDataCell>
                          <CBadge color={fingerprint.waste_score > 100 ? "danger" : "warning"}>
                            {fingerprint.waste_score.toFixed(1)}
                          </CBadge>
                        </CTableDataCell>
                      </CTableRow>
                    </CTableBody>
                  </CTable>
                </CCol>
                <CCol md={6}>
                  <h6 className="fw-bold">Tables Used</h6>
                  <div className="d-flex flex-wrap gap-2">
                    {fingerprint.tables_used?.map(t => <CBadge key={t} color="info" variant="outline">{t}</CBadge>)}
                  </div>
                </CCol>
              </CRow>
            </CTabPane>
            <CTabPane visible={activeTab === "sql"}>
              <pre className="bg-light p-3 rounded">{fingerprint.normalized_sql}</pre>
            </CTabPane>
            <CTabPane visible={activeTab === "explain"}>
              {analysis?.explain_plans?.length > 0 ? (
                 <pre className="bg-dark text-light p-3 rounded">{JSON.stringify(analysis.explain_plans[0], null, 2)}</pre>
              ) : <p className="text-muted">No EXPLAIN plans captured yet.</p>}
            </CTabPane>
          </CTabContent>
        </CCardBody>
      </CCard>
    </div>
  );
};

export default QueryFingerprintDetail;
