import axios from "axios";

// Create axios instance with default config
const api = axios.create({
  baseURL: process.env.REACT_APP_API_URL || "http://localhost:8080",
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
  const backendUrl = process.env.REACT_APP_API_URL || "http://localhost:8080";
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
    const payload = {
      resource_id: resourceId,
      workflow: workflow,
      time_range: timeRange,
      additional_context: additionalContext
    };
    
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
    throw error;
  }
};

// Ask a question about any AWS resource
export const askAwsResourceQuestion = async (resourceId, question, workflow = null) => {
  try {
    console.log(`Asking question about AWS resource ${resourceId}: ${question}`);
    const response = await api.post('/api/aws/analytics/question', {
      resource_id: resourceId,
      question: question,
      workflow: workflow
    });
    
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
export const syncAwsAccountResources = async (accountId) => {
  try {
    const response = await api.post(`/api/aws/accounts/${accountId}/sync`);
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

export default api;
