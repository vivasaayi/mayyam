import axios from 'axios';

// Set up the base URL for all API calls
const api = axios.create({
  baseURL: process.env.REACT_APP_API_URL || "http://localhost:8080",
});

// Add request interceptor to include auth token in all requests
api.interceptors.request.use(
  (config) => {
    const token = localStorage.getItem('token');
    if (token) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => Promise.reject(error)
);

const QueryTemplateService = {
  // Get all query templates
  async getAllTemplates() {
    try {
      const response = await api.get('/api/query-templates');
      return response.data;
    } catch (error) {
      console.error('Error fetching query templates:', error);
      throw error;
    }
  },

  // Get templates by connection type (mysql, postgresql, etc.)
  async getTemplatesByType(connectionType) {
    try {
      const response = await api.get(`/api/query-templates/connection-type/${connectionType}`);
      return response.data;
    } catch (error) {
      console.error(`Error fetching ${connectionType} query templates:`, error);
      throw error;
    }
  },

  // Get a specific template by ID
  async getTemplateById(id) {
    try {
      const response = await api.get(`/api/query-templates/${id}`);
      return response.data;
    } catch (error) {
      console.error('Error fetching query template:', error);
      throw error;
    }
  },

  // Create a new template
  async createTemplate(templateData) {
    try {
      const response = await api.post('/api/query-templates', templateData);
      return response.data;
    } catch (error) {
      console.error('Error creating query template:', error);
      throw error;
    }
  },

  // Update an existing template
  async updateTemplate(id, templateData) {
    try {
      const response = await api.put(`/api/query-templates/${id}`, templateData);
      return response.data;
    } catch (error) {
      console.error('Error updating query template:', error);
      throw error;
    }
  },

  // Delete a template
  async deleteTemplate(id) {
    try {
      await api.delete(`/api/query-templates/${id}`);
      return true;
    } catch (error) {
      console.error('Error deleting query template:', error);
      throw error;
    }
  }
};

export default QueryTemplateService;
