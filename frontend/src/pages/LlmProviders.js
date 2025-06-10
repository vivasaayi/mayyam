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
      const res = await fetchWithAuth("/api/v1/llm-providers", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(form),
      });
      if (!res.ok) throw new Error("Failed to create provider");
      setShowModal(false);
      setSuccess("Provider created successfully");
      fetchProviders();
    } catch (e) {
      setError(e.message);
    }
    setSaving(false);
  };

  return (
    <>
      <h2 className="mb-4">LLM Providers Management</h2>
      {error && <CAlert color="danger">{error}</CAlert>}
      {success && <CAlert color="success">{success}</CAlert>}
      <CCard className="mb-4">
        <CCardHeader>
          LLM Providers
          <CButton color="primary" className="float-end" onClick={() => setShowModal(true)}>
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
                    </CCardBody>
                  </CCard>
                </CCol>
              ))}
            </CRow>
          )}
        </CCardBody>
      </CCard>
      <CModal visible={showModal} onClose={() => setShowModal(false)}>
        <CModalHeader>Add LLM Provider</CModalHeader>
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
              <option>Custom</option>
            </CFormSelect>
            <CFormLabel className="mt-2">Model Name</CFormLabel>
            <CFormInput name="model_name" value={form.model_name} onChange={handleChange} required />
            <CFormLabel className="mt-2">API Endpoint</CFormLabel>
            <CFormInput name="api_endpoint" value={form.api_endpoint} onChange={handleChange} />
            <CFormLabel className="mt-2">API Key</CFormLabel>
            <CFormInput name="api_key" value={form.api_key} onChange={handleChange} type="password" autoComplete="new-password" />
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
              {saving ? <CSpinner size="sm" /> : "Save"}
            </CButton>
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowModal(false)}>
            Cancel
          </CButton>
        </CModalFooter>
      </CModal>
    </>
  );
};

export default LlmProviders;
