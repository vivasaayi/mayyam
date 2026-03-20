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

import { apiCall } from './api';

const BASE = '/api/chaos';

// ============================================================================
// Templates
// ============================================================================

export const listTemplates = (params = {}) => {
  const query = new URLSearchParams(params).toString();
  return apiCall(`${BASE}/templates${query ? `?${query}` : ''}`);
};

export const getTemplate = (id) => apiCall(`${BASE}/templates/${id}`);

export const createTemplate = (data) => apiCall(`${BASE}/templates`, 'POST', data);

export const updateTemplate = (id, data) => apiCall(`${BASE}/templates/${id}`, 'PUT', data);

export const deleteTemplate = (id) => apiCall(`${BASE}/templates/${id}`, 'DELETE');

export const createExperimentFromTemplate = (templateId, data) =>
  apiCall(`${BASE}/templates/${templateId}/create-experiment`, 'POST', data);

// ============================================================================
// Experiments
// ============================================================================

export const listExperiments = (params = {}) => {
  const query = new URLSearchParams(params).toString();
  return apiCall(`${BASE}/experiments${query ? `?${query}` : ''}`);
};

export const listExperimentsWithRuns = (params = {}) => {
  const query = new URLSearchParams(params).toString();
  return apiCall(`${BASE}/experiments/with-runs${query ? `?${query}` : ''}`);
};

export const getExperiment = (id) => apiCall(`${BASE}/experiments/${id}`);

export const createExperiment = (data) => apiCall(`${BASE}/experiments`, 'POST', data);

export const updateExperiment = (id, data) => apiCall(`${BASE}/experiments/${id}`, 'PUT', data);

export const deleteExperiment = (id) => apiCall(`${BASE}/experiments/${id}`, 'DELETE');

// ============================================================================
// Execution
// ============================================================================

export const runExperiment = (id, data = {}) =>
  apiCall(`${BASE}/experiments/${id}/run`, 'POST', data);

export const stopExperiment = (id) => apiCall(`${BASE}/experiments/${id}/stop`, 'POST');

export const batchRunExperiments = (data) =>
  apiCall(`${BASE}/experiments/batch-run`, 'POST', data);

// ============================================================================
// Runs & Results
// ============================================================================

export const listExperimentRuns = (experimentId) =>
  apiCall(`${BASE}/experiments/${experimentId}/runs`);

export const getRun = (runId) => apiCall(`${BASE}/runs/${runId}`);

export const getExperimentResults = (experimentId) =>
  apiCall(`${BASE}/experiments/${experimentId}/results`);

// ============================================================================
// Resource-centric
// ============================================================================

export const getExperimentsForResource = (resourceId) =>
  apiCall(`${BASE}/resources/${encodeURIComponent(resourceId)}/experiments`);

export const getResourceExperimentHistory = (resourceId) =>
  apiCall(`${BASE}/resources/${encodeURIComponent(resourceId)}/history`);
