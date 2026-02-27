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


import React, { useEffect, useState } from "react";
import {
  CCard, CCardBody, CCardHeader, CRow, CCol, CButton, CForm, CFormInput, CFormSelect, CFormLabel, CModal, CModalHeader, CModalBody, CModalFooter, CSpinner, CAlert, CBadge
} from "@coreui/react";
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { fetchWithAuth } from "../services/api";
import { useNavigate } from "react-router-dom";

const LlmProviders = () => {
  const navigate = useNavigate();
  const [providers, setProviders] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showModal, setShowModal] = useState(false);
  const [editingProvider, setEditingProvider] = useState(null);
  const [form, setForm] = useState({
    name: "",
    provider_type: "OpenAI",
    model_name: "",
    api_endpoint: "",
    api_key: "",
    prompt_format: "OpenAI",
    enabled: true,
    is_default: false,
    model_config: {},
  });
  const [saving, setSaving] = useState(false);
  const [success, setSuccess] = useState(null);
  const [deleting, setDeleting] = useState(null);
  const [testing, setTesting] = useState(null);

  const fetchProviders = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetchWithAuth("/api/v1/llm-providers");
      const data = await res.json();
      setProviders(data.providers || []);
    } catch (e) {
      setError("Failed to load LLM providers");
    }
    setLoading(false);
  };

  useEffect(() => {
    fetchProviders();
  }, []);

  const handleChange = (e) => {
    const { name, value, type, checked } = e.target;
    setForm((prev) => ({
      ...prev,
      [name]: type === "checkbox" ? checked : value,
    }));
  };

  const handleSubmit = async (e) => {
    e.preventDefault();
    setSaving(true);
    setError(null);
    setSuccess(null);
    try {
      const url = editingProvider ? `/api/v1/llm-providers/${editingProvider.id}` : "/api/v1/llm-providers";
      const method = editingProvider ? "PUT" : "POST";

      // Prepare the payload with correct field mapping
      const payload = {
        name: form.name,
        provider_type: form.provider_type,
        model_name: form.model_name,
        api_endpoint: form.api_endpoint || null,
        prompt_format: form.prompt_format,
        enabled: form.enabled,
        is_default: form.is_default,
        model_config: form.model_config,
      };

      // Only include api_key if it's provided (for security)
      if (form.api_key) {
        payload.api_key = form.api_key;
      }

      const res = await fetchWithAuth(url, {
        method,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) throw new Error(`Failed to ${editingProvider ? 'update' : 'create'} provider`);

      setShowModal(false);
      setEditingProvider(null);
      setSuccess(`Provider ${editingProvider ? 'updated' : 'created'} successfully`);
      resetForm();
      fetchProviders();
    } catch (e) {
      setError(e.message);
    }
    setSaving(false);
  };

  const resetForm = () => {
    setForm({
      name: "",
      provider_type: "OpenAI",
      model_name: "",
      api_endpoint: "",
      api_key: "",
      prompt_format: "OpenAI",
      enabled: true,
      is_default: false,
      model_config: {},
    });
  };

  const handleEdit = (provider) => {
    navigate(`/llm-providers/${provider.id}`);
  };

  const handleDelete = async (providerId) => {
    if (!window.confirm("Are you sure you want to delete this LLM provider?")) {
      return;
    }

    setDeleting(providerId);
    setError(null);
    try {
      const res = await fetchWithAuth(`/api/v1/llm-providers/${providerId}`, {
        method: "DELETE",
      });
      if (!res.ok) throw new Error("Failed to delete provider");

      setSuccess("Provider deleted successfully");
      fetchProviders();
    } catch (e) {
      setError(e.message);
    }
    setDeleting(null);
  };

  const handleCloseModal = () => {
    setShowModal(false);
    setEditingProvider(null);
    resetForm();
  };

  const handleTest = async (providerId) => {
    setTesting(providerId);
    setError(null);
    try {
      const res = await fetchWithAuth(`/api/v1/llm-providers/${providerId}/test`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ test_prompt: "Hello, this is a test message." }),
      });
      if (!res.ok) throw new Error("Failed to test provider");

      const result = await res.json();
      if (result.success) {
        setSuccess(`Test successful: ${result.message}`);
      } else {
        setError(`Test failed: ${result.message}`);
      }
    } catch (e) {
      setError(e.message);
    }
    setTesting(null);
  };

  return (
    <>
      <h2 className="mb-2">LLM Providers Management</h2>
      <p className="text-muted">We now support multiple models per provider. Use the LLM Providers list for advanced management.</p>
      {error && <CAlert color="danger">{error}</CAlert>}
      {success && <CAlert color="success">{success}</CAlert>}
      <CCard className="mb-4">
        <CCardHeader>
          LLM Providers
          <CButton color="primary" className="float-end" onClick={() => {
            resetForm();
            setEditingProvider(null);
            setShowModal(true);
          }}>
            Add Provider
          </CButton>
        </CCardHeader>
        <CCardBody>
          {loading ? (
            <CSpinner />
          ) : providers.length === 0 ? (
            <p>No LLM providers configured yet.</p>
          ) : (
            <div className="ag-theme-alpine" style={{ height: 600, width: '100%' }}>
              <AgGridReact
                rowData={providers}
                columnDefs={[
                  { headerName: 'Name', field: 'name', sortable: true, filter: true, flex: 1 },
                  { headerName: 'Provider Type', field: 'provider_type', sortable: true, filter: true },
                  { headerName: 'Default Model', field: 'model_name', sortable: true, filter: true },
                  {
                    headerName: 'Status',
                    field: 'enabled',
                    width: 120,
                    cellRenderer: params => (
                      <CBadge color={params.value ? "success" : "secondary"}>
                        {params.value ? "Enabled" : "Disabled"}
                      </CBadge>
                    )
                  },
                  {
                    headerName: 'Actions',
                    width: 250,
                    cellRenderer: params => (
                      <div className="d-flex gap-2 align-items-center h-100">
                        <CButton size="sm" color="primary" onClick={() => handleEdit(params.data)}>Edit</CButton>
                        <CButton size="sm" color="info" disabled={testing === params.data.id} onClick={() => handleTest(params.data.id)}>
                          {testing === params.data.id ? <CSpinner size="sm" /> : "Test"}
                        </CButton>
                        <CButton size="sm" color="danger" disabled={deleting === params.data.id} onClick={() => handleDelete(params.data.id)}>
                          {deleting === params.data.id ? <CSpinner size="sm" /> : "Delete"}
                        </CButton>
                      </div>
                    )
                  }
                ]}
                animateRows
                pagination
                paginationPageSize={10}
              />
            </div>
          )}
        </CCardBody>
      </CCard>
      <CModal visible={showModal} onClose={handleCloseModal}>
        <CModalHeader>{editingProvider ? 'Edit LLM Provider' : 'Add LLM Provider'}</CModalHeader>
        <CModalBody>
          <CForm onSubmit={handleSubmit}>
            <CFormLabel>Name</CFormLabel>
            <CFormInput name="name" value={form.name} onChange={handleChange} required />
            <CFormLabel className="mt-2">Provider Type</CFormLabel>
            <CFormSelect name="provider_type" value={form.provider_type} onChange={handleChange}>
              <option>OpenAI</option>
              <option>Ollama</option>
              <option>Anthropic</option>
              <option>Local</option>
              <option>Gemini</option>
              <option>DeepSeek</option>
              <option>Custom</option>
            </CFormSelect>
            <CFormLabel className="mt-2">Model Name</CFormLabel>
            <CFormInput name="model_name" value={form.model_name} onChange={handleChange} required />
            <CFormLabel className="mt-2">API Endpoint</CFormLabel>
            <CFormInput name="api_endpoint" value={form.api_endpoint} onChange={handleChange} />
            <CFormLabel className="mt-2">API Key</CFormLabel>
            <CFormInput
              name="api_key"
              value={form.api_key}
              onChange={handleChange}
              type="password"
              autoComplete="new-password"
              placeholder={editingProvider ? "Leave empty to keep existing key" : ""}
            />
            <CFormLabel className="mt-2">Prompt Format</CFormLabel>
            <CFormSelect name="prompt_format" value={form.prompt_format} onChange={handleChange}>
              <option>OpenAI</option>
              <option>Anthropic</option>
              <option>Custom</option>
            </CFormSelect>
            <CFormLabel className="mt-2">Enabled</CFormLabel>
            <CFormSelect name="enabled" value={form.enabled ? "true" : "false"} onChange={e => setForm(f => ({ ...f, enabled: e.target.value === "true" }))}>
              <option value="true">Yes</option>
              <option value="false">No</option>
            </CFormSelect>
            <CFormLabel className="mt-2">Default</CFormLabel>
            <CFormSelect name="is_default" value={form.is_default ? "true" : "false"} onChange={e => setForm(f => ({ ...f, is_default: e.target.value === "true" }))}>
              <option value="false">No</option>
              <option value="true">Yes</option>
            </CFormSelect>
            <CButton color="primary" type="submit" className="mt-3" disabled={saving}>
              {saving ? <CSpinner size="sm" /> : (editingProvider ? "Update" : "Save")}
            </CButton>
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={handleCloseModal}>
            Cancel
          </CButton>
        </CModalFooter>
      </CModal>
    </>
  );
};

export default LlmProviders;
