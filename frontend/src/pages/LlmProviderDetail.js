import React, { useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { CAlert, CBadge, CButton, CCard, CCardBody, CCardHeader, CCol, CForm, CFormInput, CFormLabel, CFormSelect, CModal, CModalBody, CModalFooter, CModalHeader, CRow, CSpinner } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-quartz.css';
import { fetchWithAuth } from '../services/api';

export default function LlmProviderDetail() {
  const { providerId } = useParams();
  const navigate = useNavigate();
  const [provider, setProvider] = useState(null);
  const [form, setForm] = useState({ name: '', provider_type: 'OpenAI', model_name: '', api_endpoint: '', prompt_format: 'OpenAI', enabled: true, is_default: false });
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);
  const [loading, setLoading] = useState(true);
  const [models, setModels] = useState([]);
  const [showModelModal, setShowModelModal] = useState(false);
  const [modelForm, setModelForm] = useState({ id: null, model_name: '', enabled: true, model_config: {} });
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);

  const load = async () => {
    setLoading(true);
    setError(null);
    const res = await fetchWithAuth(`/api/v1/llm-providers/${providerId}`);
    if (!res.ok) { setError('Failed to load provider'); setLoading(false); return; }
    const data = await res.json();
    setProvider(data);
    setForm({
      name: data.name,
      provider_type: data.provider_type,
      model_name: data.model_name,
      api_endpoint: data.api_endpoint || '',
      prompt_format: data.prompt_format,
      enabled: data.enabled,
      is_default: data.is_default,
    });
    const mr = await fetchWithAuth(`/api/v1/llm-providers/${providerId}/models`);
    if (mr.ok) {
      const mdata = await mr.json();
      setModels(mdata.models || []);
    }
    setLoading(false);
  };

  useEffect(() => { load(); }, [providerId]);

  const save = async (e) => {
    e.preventDefault(); setSaving(true); setError(null); setSuccess(null);
    const payload = { name: form.name, model_name: form.model_name, api_endpoint: form.api_endpoint || null, prompt_format: form.prompt_format, enabled: form.enabled, is_default: form.is_default };
    const res = await fetchWithAuth(`/api/v1/llm-providers/${providerId}`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    if (!res.ok) setError('Failed to save'); else { setSuccess('Saved'); load(); }
    setSaving(false);
  };

  const testProvider = async () => {
    setTesting(true); setError(null); setSuccess(null);
    const res = await fetchWithAuth(`/api/v1/llm-providers/${providerId}/test`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ test_prompt: 'What is the weather in San Francisco today?' }) });
    const body = await res.json().catch(() => ({}));
    if (res.ok && body.success) setSuccess(`Test OK: ${body.message}`); else setError(`Test failed: ${body.message || res.status}`);
    setTesting(false);
  };

  const cols = useMemo(() => [
    { headerName: 'Model', field: 'model_name', sortable: true, filter: true },
    { headerName: 'Enabled', field: 'enabled', width: 110, valueFormatter: p => p.value ? 'Yes' : 'No' },
    {
      headerName: 'Actions', width: 280, cellRenderer: params => (
        <div style={{ display: 'flex', gap: 8 }}>
          <CButton size="sm" onClick={() => { setModelForm({ id: params.data.id, model_name: params.data.model_name, enabled: params.data.enabled, model_config: params.data.model_config || {} }); setShowModelModal(true); }}>Edit</CButton>
          <CButton size="sm" color={params.data.enabled ? 'secondary' : 'success'} onClick={async () => {
            const res = await fetchWithAuth(`/api/v1/llm-providers/${providerId}/models/${params.data.id}/toggle?enabled=${!params.data.enabled}`, { method: 'POST' });
            if (res.ok) load();
          }}>{params.data.enabled ? 'Disable' : 'Enable'}</CButton>
          <CButton size="sm" color="danger" onClick={async () => {
            if (!window.confirm('Delete model?')) return;
            const res = await fetchWithAuth(`/api/v1/llm-providers/${providerId}/models/${params.data.id}`, { method: 'DELETE' });
            if (res.ok) load();
          }}>Delete</CButton>
        </div>
      )
    }
  ], [providerId]);

  const saveModel = async (e) => {
    e.preventDefault();
    const body = { model_name: modelForm.model_name, model_config: modelForm.model_config, enabled: modelForm.enabled };
    const url = modelForm.id
      ? `/api/v1/llm-providers/${providerId}/models/${modelForm.id}`
      : `/api/v1/llm-providers/${providerId}/models`;
    const method = modelForm.id ? 'PUT' : 'POST';
    const res = await fetchWithAuth(url, { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
    if (!res.ok) { setError('Failed to save model'); return; }
    setShowModelModal(false); setModelForm({ id: null, model_name: '', enabled: true, model_config: {} }); load();
  };

  if (loading) return <CSpinner />;
  if (error) return <CAlert color="danger">{error}</CAlert>;
  if (!provider) return null;

  return (
    <>
      {success && <CAlert color="success">{success}</CAlert>}
      <CButton color="secondary" size="sm" onClick={() => navigate('/llm-providers')}>Back</CButton>
      <CCard className="mt-3">
        <CCardHeader className="d-flex align-items-center justify-content-between">
          <div>
            <strong>{provider.name}</strong> <CBadge color={form.enabled ? 'success' : 'secondary'}>{form.enabled ? 'Enabled' : 'Disabled'}</CBadge>
          </div>
          <CButton size="sm" color="info" disabled={testing} onClick={testProvider}>{testing ? 'Testing…' : 'Test'}</CButton>
        </CCardHeader>
        <CCardBody>
          <CForm onSubmit={save}>
            <CRow className="g-3">
              <CCol md={4}><CFormLabel>Name</CFormLabel><CFormInput value={form.name} onChange={e => setForm(f => ({...f, name: e.target.value}))} required /></CCol>
              <CCol md={3}><CFormLabel>Type</CFormLabel><CFormInput value={form.provider_type} disabled /></CCol>
              <CCol md={3}><CFormLabel>Default Model</CFormLabel><CFormInput value={form.model_name} onChange={e => setForm(f => ({...f, model_name: e.target.value}))} required /></CCol>
              <CCol md={6}><CFormLabel>API Endpoint</CFormLabel><CFormInput value={form.api_endpoint} onChange={e => setForm(f => ({...f, api_endpoint: e.target.value}))} /></CCol>
              <CCol md={3}><CFormLabel>Prompt Format</CFormLabel><CFormSelect value={form.prompt_format} onChange={e => setForm(f => ({...f, prompt_format: e.target.value}))}><option>OpenAI</option><option>Anthropic</option><option>Custom</option></CFormSelect></CCol>
              <CCol md={2}><CFormLabel>Enabled</CFormLabel><CFormSelect value={form.enabled ? 'true' : 'false'} onChange={e => setForm(f => ({...f, enabled: e.target.value === 'true'}))}><option value="true">Yes</option><option value="false">No</option></CFormSelect></CCol>
              <CCol md={2}><CFormLabel>Default</CFormLabel><CFormSelect value={form.is_default ? 'true' : 'false'} onChange={e => setForm(f => ({...f, is_default: e.target.value === 'true'}))}><option value="false">No</option><option value="true">Yes</option></CFormSelect></CCol>
            </CRow>
            <CButton className="mt-3" disabled={saving} type="submit">{saving ? 'Saving…' : 'Save'}</CButton>
          </CForm>
        </CCardBody>
      </CCard>

      <CCard className="mt-3">
        <CCardHeader className="d-flex justify-content-between align-items-center">
          <div>Models</div>
          <CButton size="sm" onClick={() => { setModelForm({ id: null, model_name: '', enabled: true, model_config: {} }); setShowModelModal(true); }}>Add Model</CButton>
        </CCardHeader>
        <CCardBody>
          <div className="ag-theme-quartz" style={{ height: 420, width: '100%' }}>
            <AgGridReact rowData={models} columnDefs={cols} animateRows pagination />
          </div>
        </CCardBody>
      </CCard>

      <CModal visible={showModelModal} onClose={() => setShowModelModal(false)}>
        <CModalHeader>{modelForm.id ? 'Edit Model' : 'Add Model'}</CModalHeader>
        <CModalBody>
          <CForm onSubmit={saveModel}>
            <CFormLabel>Model Name</CFormLabel>
            <CFormInput value={modelForm.model_name} onChange={e => setModelForm(m => ({...m, model_name: e.target.value}))} required />
            <CButton className="mt-3" type="submit">Save</CButton>
          </CForm>
        </CModalBody>
        <CModalFooter><CButton color="secondary" onClick={() => setShowModelModal(false)}>Close</CButton></CModalFooter>
      </CModal>
    </>
  );
}
