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


import React, { useEffect, useMemo, useState } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton, CCard, CCardBody, CCardHeader } from '@coreui/react';
import { useNavigate } from 'react-router-dom';
import { fetchWithAuth } from '../services/api';

export default function LlmProvidersList() {
  const [rowData, setRowData] = useState([]);
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();

  const columnDefs = useMemo(() => [
    { headerName: 'Name', field: 'name', sortable: true, filter: true },
    { headerName: 'Type', field: 'provider_type', width: 130 },
    { headerName: 'Model', field: 'model_name', width: 160 },
    { headerName: 'Default', field: 'is_default', width: 110, valueFormatter: p => p.value ? 'Yes' : 'No' },
    { headerName: 'Enabled', field: 'enabled', width: 110, valueFormatter: p => p.value ? 'Yes' : 'No' },
    {
      headerName: 'Actions', width: 220, cellRenderer: params => (
        <div style={{ display: 'flex', gap: 8 }}>
          <CButton size="sm" onClick={() => navigate(`/llm-providers/${params.data.id}`)}>Edit</CButton>
          <CButton size="sm" color="danger" onClick={async () => {
            if (!window.confirm('Delete provider?')) return;
            const res = await fetchWithAuth(`/api/v1/llm-providers/${params.data.id}`, { method: 'DELETE' });
            if (res.ok) load();
          }}>Delete</CButton>
        </div>
      )
    }
  ], [navigate]);

  const load = async () => {
    setLoading(true);
    const res = await fetchWithAuth('/api/v1/llm-providers');
    if (res.ok) {
      const data = await res.json();
      setRowData(data.providers || []);
    }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  return (
    <CCard>
      <CCardHeader className="d-flex justify-content-between align-items-center">
        <div>LLM Providers</div>
        <CButton size="sm" onClick={() => navigate('/llm-providers/new')}>Add Provider</CButton>
      </CCardHeader>
      <CCardBody>
        <div className="ag-theme-alpine" style={{ height: 480, width: '100%' }}>
          <AgGridReact rowData={rowData} columnDefs={columnDefs} animateRows pagination />
        </div>
      </CCardBody>
    </CCard>
  );
}
