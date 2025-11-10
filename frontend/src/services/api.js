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


import axios from "axios";

// Create axios instance with default config
const api = axios.create({
  baseURL: process.env.REACT_APP_API_URL || "http://localhost:8010",
  headers: {
    "Content-Type": "application/json",
  },
});

// Add a request interceptor to add the auth token to every request
api.interceptors.request.use(
  (config) => {
    const token = localStorage.getItem("token");
    if (token) {
      config.headers["Authorization"] = `Bearer ${token}`;
    }
    return config;
  },
  (error) => {
    return Promise.reject(error);
  }
);

// Add a response interceptor to handle common errors
api.interceptors.response.use(
  (response) => {
    return response;
  },
  (error) => {
    // Handle 401 Unauthorized errors by logging the user out
    if (error.response && error.response.status === 401) {
      localStorage.removeItem("token");
      window.location.href = "/login";
    }
    return Promise.reject(error);
  }
);

// Fetch with authentication included
export const fetchWithAuth = async (url, options = {}) => {
  const token = localStorage.getItem("token");
  const backendUrl = process.env.REACT_APP_API_URL || "http://localhost:8010";
  const fullUrl = url.startsWith('http') ? url : `${backendUrl}${url}`;
  
  const headers = {
    ...options.headers,
  };
  
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }
  
  console.log(`Making request to: ${fullUrl}`);
  const response = await fetch(fullUrl, {
    ...options,
    headers,
  });
  
  // Handle authentication errors
  if (response.status === 401) {
    localStorage.removeItem("token");
    window.location.href = "/login";
  }
  
  return response;
};

// Generic API call function
export const apiCall = async (url, method = 'GET', data = null) => {
  try {
    const config = {
      method,
      url,
    };

    if (data && (method === 'POST' || method === 'PUT' || method === 'PATCH')) {
      config.data = data;
    }

    const response = await api(config);
    return response;
  } catch (error) {
    console.error(`API call error for ${url}:`, error);
    throw error;
  }
};

// Analyze RDS instance with specified workflow
export const analyzeRdsInstance = async (instanceId, workflow) => {
  try {
    const response = await api.get(`/api/ai/analyze/rds/${instanceId}/${workflow}`);
    
    // Ensure the response has related questions
    if (!response.data.relatedQuestions || response.data.relatedQuestions.length === 0) {
      console.log("Adding default related questions to response");
      response.data.relatedQuestions = [
        "How can I optimize my memory configuration?",
        "Is my current memory allocation sufficient?",
        "What are the peak memory usage patterns?"
      ];
    }
    
    return response.data;
  } catch (error) {
    console.error("Error analyzing RDS instance:", error);
    throw error;
  }
};

// Ask a related question about an RDS instance
export const askRdsQuestion = async (instanceId, question, workflow = null) => {
  try {
    console.log(`Asking question about RDS instance ${instanceId}: ${question}`);
    const response = await api.post('/api/ai/analyze/rds/question', {
      instance_id: instanceId,
      question: question,
      workflow: workflow
    });
    
    console.log("Question response received:", response.data);
    
    // Ensure the response has related questions
    if (!response.data.relatedQuestions || response.data.relatedQuestions.length === 0) {
      console.log("Adding default related questions to follow-up response");
      response.data.relatedQuestions = [
        "How does this compare to other similar workloads?",
        "What metrics should I monitor after applying these changes?",
        "How can I automate this optimization process?"
      ];
    }
    
    return response.data;
  } catch (error) {
    console.error("Error asking RDS question:", error);
    throw error;
  }
};

// Fetch all AWS accounts
export const getAwsAccounts = async () => {
  try {
    const response = await api.get('/api/aws/accounts');
    return response.data;
  } catch (error) {
    console.error("Error fetching AWS accounts:", error);
    throw error;
  }
};

// Analyze any AWS resource
export const analyzeAwsResource = async (resourceId, workflow, timeRange = null, additionalContext = null) => {
  try {
    if (!resourceId) {
      throw new Error("Resource ID is required for analysis");
    }
    
    if (!workflow) {
      throw new Error("Workflow ID is required for resource analysis");
    }
    
    // Only include fields that are expected by the backend
    const payload = {
      resource_id: resourceId,
      workflow: workflow
    };
    
    // Only add time_range if it's provided
    if (timeRange) {
      payload.time_range = timeRange;
    }
    
    console.log("Sending analyze request payload:", JSON.stringify(payload));
    const response = await api.post('/api/aws/analytics/analyze', payload);
    
    // Ensure the response has related questions
    if (!response.data.related_questions || response.data.related_questions.length === 0) {
      console.log("Adding default related questions to resource analysis response");
      response.data.related_questions = [
        "How can I optimize this resource?",
        "What are the best practices for this resource type?",
        "Are there any performance concerns I should address?"
      ];
    }
    
    return response.data;
  } catch (error) {
    console.error("Error analyzing AWS resource:", error);
    // Add additional logging for debugging
    if (error.response) {
      console.error("Response error data:", error.response.data);
      console.error("Response status:", error.response.status);
      console.error("Response headers:", error.response.headers);
    } else if (error.request) {
      console.error("Request was made but no response received:", error.request);
    } else {
      console.error("Error setting up request:", error.message);
    }
    throw error;
  }
};

// Ask a question about any AWS resource
export const askAwsResourceQuestion = async (resourceId, question, workflow = null) => {
  try {
    console.log(`Asking question about AWS resource ${resourceId}: ${question}`);
    
    // Check if resourceId is valid
    if (!resourceId || resourceId.trim() === '') {
      throw new Error('Resource ID is missing or empty');
    }
    
    // Check if question is valid
    if (!question || question.trim() === '') {
      throw new Error('Question text is missing or empty');
    }
    
    // Capture request payload for debugging
    const payload = {
      resource_id: resourceId,
      question: question,
      workflow: workflow
    };
    
    console.log("Question API request payload:", JSON.stringify(payload));
    const response = await api.post('/api/aws/analytics/question', payload);
    
    console.log("Question response received:", response.data);
    
    // Ensure the response has related questions
    if (!response.data.related_questions || response.data.related_questions.length === 0) {
      console.log("Adding default related questions to follow-up response");
      response.data.related_questions = [
        "How does this compare to similar resources?",
        "What metrics should I monitor going forward?",
        "How can I automate this optimization process?"
      ];
    }
    
    return response.data;
  } catch (error) {
    console.error("Error asking AWS resource question:", error);
    
    // Provide more detailed error logging
    if (error.response) {
      console.error("API response error:", {
        status: error.response.status,
        statusText: error.response.statusText,
        data: error.response.data
      });
      
      // If the backend provides a specific error message, use it
      if (error.response.data && error.response.data.message) {
        throw new Error(`API Error: ${error.response.data.message}`);
      }
    }
    
    throw error;
  }
};

// Get specific AWS account by ID
export const getAwsAccountById = async (accountId) => {
  try {
    const response = await api.get(`/api/aws/accounts/${accountId}`);
    return response.data;
  } catch (error) {
    console.error(`Error fetching AWS account ${accountId}:`, error);
    throw error;
  }
};

// Create a new AWS account
export const createAwsAccount = async (accountData) => {
  try {
    const response = await api.post('/api/aws/accounts', accountData);
    return response.data;
  } catch (error) {
    console.error("Error creating AWS account:", error);
    throw error;
  }
};

// Update an existing AWS account
export const updateAwsAccount = async (accountId, accountData) => {
  try {
    const response = await api.put(`/api/aws/accounts/${accountId}`, accountData);
    return response.data;
  } catch (error) {
    console.error(`Error updating AWS account ${accountId}:`, error);
    throw error;
  }
};

// Delete an AWS account
export const deleteAwsAccount = async (accountId) => {
  try {
    const response = await api.delete(`/api/aws/accounts/${accountId}`);
    return response.status === 204; // Return true if successfully deleted
  } catch (error) {
    console.error(`Error deleting AWS account ${accountId}:`, error);
    throw error;
  }
};

// Sync resources for an AWS account
export const syncAwsAccountResources = async (accountId, syncId = null) => {
  try {
    const params = syncId ? { params: { sync_id: syncId } } : undefined;
    const response = await api.post(`/api/aws/accounts/${accountId}/sync`, null, params);
    return response.data;
  } catch (error) {
    console.error(`Error syncing resources for AWS account ${accountId}:`, error);
    throw error;
  }
};

// Sync resources for all AWS accounts
export const syncAllAwsAccountResources = async () => {
  try {
    const response = await api.post('/api/aws/accounts/sync');
    return response.data;
  } catch (error) {
    console.error('Error syncing resources for all AWS accounts:', error);
    throw error;
  }
};

// Sync Runs API
export const createSyncRun = async ({ name, aws_account_id = null, account_id = null, profile = null, region = null, metadata = {} }) => {
  try {
    const response = await api.post('/api/sync-runs', { name, aws_account_id, account_id, profile, region, metadata });
    return response.data; // returns SyncRunDto including id (sync_id)
  } catch (error) {
    console.error('Error creating sync run:', error);
    throw error;
  }
};

export const getSyncRuns = async (status = null, limit = 50, offset = 0) => {
  try {
    const params = {};
    if (status) params.status = status;
    if (limit != null) params.limit = limit;
    if (offset != null) params.offset = offset;
    const response = await api.get('/api/sync-runs', { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching sync runs:', error);
    throw error;
  }
};

export const getSyncRun = async (id) => {
  try {
    const response = await api.get(`/api/sync-runs/${id}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching sync run:', error);
    throw error;
  }
};

// AWS Regions (live via DescribeRegions)
export const listAwsRegions = async ({ accountId = null, profile = null, region = null } = {}) => {
  try {
    const params = {};
    if (accountId) params.account_id = accountId;
    if (profile) params.profile = profile;
    if (region) params.region = region;
    const response = await api.get('/api/aws/regions', { params });
    return response.data?.regions || [];
  } catch (error) {
    console.error('Error listing AWS regions:', error);
    throw error;
  }
};

export default api;

// Performance Analysis API functions

// Aurora Clusters
export const getAuroraClusters = async () => {
  try {
    const response = await api.get('/api/aurora-clusters');
    return response.data;
  } catch (error) {
    console.error('Error fetching Aurora clusters:', error);
    throw error;
  }
};

export const getAuroraCluster = async (id) => {
  try {
    const response = await api.get(`/api/aurora-clusters/${id}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching Aurora cluster:', error);
    throw error;
  }
};

export const createAuroraCluster = async (clusterData) => {
  try {
    const response = await api.post('/api/aurora-clusters', clusterData);
    return response.data;
  } catch (error) {
    console.error('Error creating Aurora cluster:', error);
    throw error;
  }
};

export const updateAuroraCluster = async (id, clusterData) => {
  try {
    const response = await api.put(`/api/aurora-clusters/${id}`, clusterData);
    return response.data;
  } catch (error) {
    console.error('Error updating Aurora cluster:', error);
    throw error;
  }
};

export const deleteAuroraCluster = async (id) => {
  try {
    const response = await api.delete(`/api/aurora-clusters/${id}`);
    return response.data;
  } catch (error) {
    console.error('Error deleting Aurora cluster:', error);
    throw error;
  }
};

export const testAuroraClusterConnection = async (id) => {
  try {
    const response = await api.post(`/api/aurora-clusters/${id}/test-connection`);
    return response.data;
  } catch (error) {
    console.error('Error testing Aurora cluster connection:', error);
    throw error;
  }
};

// Slow Query Events
export const getSlowQueryEvents = async (params = {}) => {
  try {
    const response = await api.get('/api/slow-queries', { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching slow query events:', error);
    throw error;
  }
};

export const getSlowQueryEvent = async (id) => {
  try {
    const response = await api.get(`/api/slow-queries/${id}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching slow query event:', error);
    throw error;
  }
};

export const analyzeSlowQueries = async (clusterId, params = {}) => {
  try {
    const response = await api.post(`/api/slow-queries/analyze/${clusterId}`, params);
    return response.data;
  } catch (error) {
    console.error('Error analyzing slow queries:', error);
    throw error;
  }
};

export const getSlowQueryStatistics = async (clusterId, params = {}) => {
  try {
    const response = await api.get(`/api/slow-queries/statistics/${clusterId}`, { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching slow query statistics:', error);
    throw error;
  }
};

// Query Fingerprints
export const getQueryFingerprints = async (params = {}) => {
  try {
    const response = await api.get('/api/query-fingerprints', { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching query fingerprints:', error);
    throw error;
  }
};

export const getQueryFingerprint = async (id) => {
  try {
    const response = await api.get(`/api/query-fingerprints/${id}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching query fingerprint:', error);
    throw error;
  }
};

export const getFingerprintAnalysis = async (id) => {
  try {
    const response = await api.get(`/api/query-fingerprints/${id}/analysis`);
    return response.data;
  } catch (error) {
    console.error('Error fetching fingerprint analysis:', error);
    throw error;
  }
};

export const getFingerprintPatterns = async (clusterId) => {
  try {
    const response = await api.get(`/api/query-fingerprints/patterns/${clusterId}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching fingerprint patterns:', error);
    throw error;
  }
};

// Explain Plans
export const getExplainPlans = async (params = {}) => {
  try {
    const response = await api.get('/api/explain-plans', { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching explain plans:', error);
    throw error;
  }
};

export const getExplainPlan = async (id) => {
  try {
    const response = await api.get(`/api/explain-plans/${id}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching explain plan:', error);
    throw error;
  }
};

export const createExplainPlan = async (planData) => {
  try {
    const response = await api.post('/api/explain-plans', planData);
    return response.data;
  } catch (error) {
    console.error('Error creating explain plan:', error);
    throw error;
  }
};

export const compareExplainPlans = async (planIds) => {
  try {
    const response = await api.post('/api/explain-plans/compare', { plan_ids: planIds });
    return response.data;
  } catch (error) {
    console.error('Error comparing explain plans:', error);
    throw error;
  }
};

export const getPlanOptimization = async (id) => {
  try {
    const response = await api.get(`/api/explain-plans/${id}/optimization`);
    return response.data;
  } catch (error) {
    console.error('Error fetching plan optimization:', error);
    throw error;
  }
};

// AI Analysis
export const getAiAnalyses = async (params = {}) => {
  try {
    const response = await api.get('/api/ai-analysis', { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching AI analyses:', error);
    throw error;
  }
};

export const getAiAnalysis = async (id) => {
  try {
    const response = await api.get(`/api/ai-analysis/${id}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching AI analysis:', error);
    throw error;
  }
};

export const generateAiAnalysis = async (analysisData) => {
  try {
    const response = await api.post('/api/ai-analysis/generate', analysisData);
    return response.data;
  } catch (error) {
    console.error('Error generating AI analysis:', error);
    throw error;
  }
};

export const getAiInsights = async (clusterId, params = {}) => {
  try {
    const response = await api.get(`/api/ai-analysis/insights/${clusterId}`, { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching AI insights:', error);
    throw error;
  }
};

// Performance Monitoring
export const getPerformanceSnapshots = async (params = {}) => {
  try {
    const response = await api.get('/api/performance-monitoring/snapshots', { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching performance snapshots:', error);
    throw error;
  }
};

export const getPerformanceSnapshot = async (id) => {
  try {
    const response = await api.get(`/api/performance-monitoring/snapshots/${id}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching performance snapshot:', error);
    throw error;
  }
};

export const createPerformanceSnapshot = async (snapshotData) => {
  try {
    const response = await api.post('/api/performance-monitoring/snapshots', snapshotData);
    return response.data;
  } catch (error) {
    console.error('Error creating performance snapshot:', error);
    throw error;
  }
};

export const getPerformanceHealthScore = async (clusterId) => {
  try {
    const response = await api.get(`/api/performance-monitoring/health/${clusterId}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching performance health score:', error);
    throw error;
  }
};

export const getPerformanceTrends = async (clusterId, params = {}) => {
  try {
    const response = await api.get(`/api/performance-monitoring/trends/${clusterId}`, { params });
    return response.data;
  } catch (error) {
    console.error('Error fetching performance trends:', error);
    throw error;
  }
};

export const getPerformanceAlerts = async (clusterId) => {
  try {
    const response = await api.get(`/api/performance-monitoring/alerts/${clusterId}`);
    return response.data;
  } catch (error) {
    console.error('Error fetching performance alerts:', error);
    throw error;
  }
};
