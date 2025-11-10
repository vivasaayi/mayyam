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
import {
  CCard,
  CCardBody,
  CCardHeader,
  CButton,
  CModal,
  CModalHeader,
  CModalTitle,
  CModalBody,
  CModalFooter,
  CForm,
  CFormLabel,
  CFormInput,
  CFormTextarea,
  CFormSelect,
  CFormCheck,
  CSpinner,
  CRow,
  CCol,
  CAlert
} from "@coreui/react";
import QueryTemplateService from "../../services/queryTemplateService";

const QueryTemplateManager = ({ connection, onTemplateSelect, initialTemplates = [] }) => {
  const [templates, setTemplates] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showModal, setShowModal] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState(null);
  const [formData, setFormData] = useState({
    name: "",
    query: "",
    description: "",
    connection_type: "",
    category: "",
    is_favorite: false,
    display_order: 999
  });

  // Categories for query templates
  const categories = [
    { value: "Performance", label: "Performance Analysis" },
    { value: "Schema", label: "Schema Information" },
    { value: "Monitoring", label: "Database Monitoring" },
    { value: "Statistics", label: "Statistics" },
    { value: "Maintenance", label: "Maintenance" },
    { value: "Other", label: "Other" }
  ];

  // Use initialTemplates if provided on first load
  useEffect(() => {
    if (initialTemplates && initialTemplates.length > 0) {
      setTemplates(initialTemplates);
      setLoading(false);
    } else if (connection) {
      loadTemplates();
    }
  }, [connection, initialTemplates]);

  // Load templates from the backend
  const loadTemplates = async () => {
    try {
      setLoading(true);
      setError(null);
      let data;
      // For common templates (empty connection_type)
      if (!connection.connection_type) {
        data = await QueryTemplateService.getCommonTemplates();
      } else {
        data = await QueryTemplateService.getTemplatesByType(connection.connection_type);
      }
      setTemplates(data);
    } catch (err) {
      console.error("Failed to load templates:", err);
      setError("Failed to load query templates. Please try again.");
    } finally {
      setLoading(false);
    }
  };

  // Open modal to create new template
  const handleNewTemplate = () => {
    setEditingTemplate(null);
    setFormData({
      name: "",
      query: "",
      description: "",
      connection_type: connection.connection_type,
      category: "",
      is_favorite: false,
      display_order: 999
    });
    setShowModal(true);
  };

  // Open modal to edit existing template
  const handleEditTemplate = (template) => {
    setEditingTemplate(template);
    setFormData({
      name: template.name,
      query: template.query,
      description: template.description || "",
      connection_type: template.connection_type,
      category: template.category || "",
      is_favorite: template.is_favorite,
      display_order: template.display_order
    });
    setShowModal(true);
  };

  // Handle form input changes
  const handleInputChange = (e) => {
    const { name, value, type, checked } = e.target;
    setFormData({
      ...formData,
      [name]: type === "checkbox" ? checked : value
    });
  };

  // Save template (create or update)
  const handleSaveTemplate = async () => {
    try {
      setLoading(true);
      
      if (editingTemplate) {
        // Update existing template
        await QueryTemplateService.updateTemplate(editingTemplate.id, formData);
      } else {
        // Create new template
        await QueryTemplateService.createTemplate(formData);
      }
      
      // Reload templates and close modal
      await loadTemplates();
      setShowModal(false);
    } catch (err) {
      console.error("Failed to save template:", err);
      setError("Failed to save query template. Please try again.");
    } finally {
      setLoading(false);
    }
  };

  // Delete a template
  const handleDeleteTemplate = async (template) => {
    if (window.confirm(`Are you sure you want to delete the template "${template.name}"?`)) {
      try {
        setLoading(true);
        await QueryTemplateService.deleteTemplate(template.id);
        await loadTemplates();
      } catch (err) {
        console.error("Failed to delete template:", err);
        setError("Failed to delete query template. Please try again.");
      } finally {
        setLoading(false);
      }
    }
  };

  // Group templates by category
  const groupedTemplates = templates.reduce((acc, template) => {
    const category = template.category || "Other";
    if (!acc[category]) {
      acc[category] = [];
    }
    acc[category].push(template);
    return acc;
  }, {});

  return (
    <div>
      {error && <CAlert color="danger">{error}</CAlert>}
      
      <div className="d-flex justify-content-between align-items-center mb-3">
        <h5>Query Templates</h5>
        <CButton color="primary" size="sm" onClick={handleNewTemplate}>
          + New Template
        </CButton>
      </div>

      {loading && !templates.length ? (
        <div className="text-center my-4">
          <CSpinner color="primary" />
        </div>
      ) : templates.length === 0 ? (
        <CAlert color="info">
          No query templates found for {connection.connection_type}. Create your first template to get started.
        </CAlert>
      ) : (
        Object.entries(groupedTemplates).map(([category, categoryTemplates]) => (
          <div key={category} className="mb-4">
            <h6 className="border-bottom pb-2">{category}</h6>
            <CRow>
              {categoryTemplates.map((template) => (
                <CCol lg={6} key={template.id} className="mb-3">
                  <CCard>
                    <CCardHeader className="d-flex justify-content-between align-items-center py-2">
                      <div>
                        <strong>{template.name}</strong>
                        {template.is_favorite && <span className="ms-2">⭐</span>}
                      </div>
                      <div>
                        <CButton
                          color="light"
                          size="sm"
                          className="me-1"
                          onClick={() => handleEditTemplate(template)}
                        >
                          Edit
                        </CButton>
                        <CButton
                          color="primary"
                          size="sm"
                          className="me-1"
                          onClick={() => onTemplateSelect(template.query)}
                        >
                          Use
                        </CButton>
                        <CButton
                          color="danger"
                          size="sm"
                          variant="ghost"
                          onClick={() => handleDeleteTemplate(template)}
                        >
                          Delete
                        </CButton>
                      </div>
                    </CCardHeader>
                    <CCardBody className="py-2">
                      {template.description && (
                        <p className="text-muted small mb-2">{template.description}</p>
                      )}
                      <pre className="bg-light p-2 small" style={{ maxHeight: "150px", overflow: "auto" }}>
                        {template.query}
                      </pre>
                    </CCardBody>
                  </CCard>
                </CCol>
              ))}
            </CRow>
          </div>
        ))
      )}

      {/* Modal for creating/editing templates */}
      <CModal visible={showModal} onClose={() => setShowModal(false)} size="lg">
        <CModalHeader onClose={() => setShowModal(false)}>
          <CModalTitle>{editingTemplate ? "Edit Query Template" : "New Query Template"}</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <CForm>
            <CRow className="mb-3">
              <CCol md={8}>
                <CFormLabel htmlFor="name">Template Name</CFormLabel>
                <CFormInput
                  id="name"
                  name="name"
                  value={formData.name}
                  onChange={handleInputChange}
                  required
                />
              </CCol>
              <CCol md={4}>
                <CFormLabel htmlFor="category">Category</CFormLabel>
                <CFormSelect
                  id="category"
                  name="category"
                  value={formData.category}
                  onChange={handleInputChange}
                >
                  <option value="">Select a category</option>
                  {categories.map((cat) => (
                    <option key={cat.value} value={cat.value}>
                      {cat.label}
                    </option>
                  ))}
                </CFormSelect>
              </CCol>
            </CRow>
            
            <div className="mb-3">
              <CFormLabel htmlFor="description">Description (optional)</CFormLabel>
              <CFormInput
                id="description"
                name="description"
                value={formData.description}
                onChange={handleInputChange}
              />
            </div>
            
            <div className="mb-3">
              <CFormLabel htmlFor="query">SQL Query</CFormLabel>
              <CFormTextarea
                id="query"
                name="query"
                value={formData.query}
                onChange={handleInputChange}
                rows={8}
                style={{ fontFamily: "Monaco, 'Courier New', monospace" }}
                required
              />
            </div>
            
            <CRow className="mb-3">
              <CCol md={6}>
                <CFormLabel htmlFor="connection_type">Database Type</CFormLabel>
                <CFormInput
                  id="connection_type"
                  name="connection_type"
                  value={formData.connection_type}
                  onChange={handleInputChange}
                  disabled
                />
              </CCol>
              <CCol md={3}>
                <CFormLabel htmlFor="display_order">Display Order</CFormLabel>
                <CFormInput
                  type="number"
                  id="display_order"
                  name="display_order"
                  value={formData.display_order}
                  onChange={handleInputChange}
                />
              </CCol>
              <CCol md={3} className="d-flex align-items-end">
                <CFormCheck
                  id="is_favorite"
                  name="is_favorite"
                  label="Favorite"
                  checked={formData.is_favorite}
                  onChange={handleInputChange}
                />
              </CCol>
            </CRow>
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowModal(false)}>
            Cancel
          </CButton>
          <CButton color="primary" onClick={handleSaveTemplate} disabled={loading}>
            {loading ? <CSpinner size="sm" /> : "Save Template"}
          </CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default QueryTemplateManager;
