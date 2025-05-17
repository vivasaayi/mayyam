import axios from "axios";

// Create axios instance with default config
const api = axios.create({
  baseURL: process.env.REACT_APP_API_URL || "",
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
  
  const headers = {
    ...options.headers,
  };
  
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }
  
  const response = await fetch(url, {
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

export default api;
