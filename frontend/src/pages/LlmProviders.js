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
import { fetchWithAuth } from "../services/api";

const LlmProviders = () => {
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
    setEditingProvider(provider);
    setForm({
      name: provider.name,
      provider_type: provider.provider_type,
      model_name: provider.model_name,
      api_endpoint: provider.base_url || "",
      api_key: "", // Don't populate for security
      prompt_format: provider.prompt_format,
      enabled: provider.enabled,
      is_default: provider.is_default,
      model_config: provider.model_config || {},
    });
    setShowModal(true);
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
            <CRow>
              {providers.map((prov) => (
                <CCol md={6} lg={4} key={prov.id} className="mb-3">
                  <CCard>
                    <CCardHeader>
                      <strong>{prov.name}</strong> <CBadge color={prov.enabled ? "success" : "secondary"}>{prov.enabled ? "Enabled" : "Disabled"}</CBadge>
                    </CCardHeader>
                    <CCardBody>
                      <div><b>Type:</b> {prov.provider_type}</div>
                      <div><b>Model:</b> {prov.model_name}</div>
                      <div><b>Default:</b> {prov.is_default ? "Yes" : "No"}</div>
                      <div><b>Created:</b> {new Date(prov.created_at).toLocaleString()}</div>
                      
                      <div className="mt-3 d-flex gap-2 flex-wrap">
                        <CButton 
                          color="primary" 
                          size="sm" 
                          onClick={() => handleEdit(prov)}
                        >
                          Edit
                        </CButton>
                        <CButton 
                          color="info" 
                          size="sm" 
                          onClick={() => handleTest(prov.id)}
                          disabled={testing === prov.id}
                        >
                          {testing === prov.id ? <CSpinner size="sm" /> : "Test"}
                        </CButton>
                        <CButton 
                          color="danger" 
                          size="sm" 
                          onClick={() => handleDelete(prov.id)}
                          disabled={deleting === prov.id}
                        >
                          {deleting === prov.id ? <CSpinner size="sm" /> : "Delete"}
                        </CButton>
                      </div>
                    </CCardBody>
                  </CCard>
                </CCol>
              ))}
            </CRow>
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
