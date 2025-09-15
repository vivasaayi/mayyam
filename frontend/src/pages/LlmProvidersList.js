import React, { useEffect, useMemo, useState } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-quartz.css';
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
        <div className="ag-theme-quartz" style={{ height: 480, width: '100%' }}>
          <AgGridReact rowData={rowData} columnDefs={columnDefs} animateRows pagination />
        </div>
      </CCardBody>
    </CCard>
  );
}
