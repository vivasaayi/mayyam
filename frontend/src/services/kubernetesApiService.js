// This service will handle all API calls to your backend for Kubernetes data.

// Base URL for the backend API
// Assuming the backend is running on port 8080 locally
const API_BASE_URL = 'http://localhost:8080/api/kubernetes';

// Helper function to handle fetch responses
const handleResponse = async (response) => {
    if (!response.ok) {
        const errorData = await response.json().catch(() => ({ message: 'Network response was not ok and failed to parse error JSON.' }));
        console.error('API Error:', response.status, errorData);
        throw new Error(errorData.message || `API request failed with status ${response.status}`);
    }
    return response.json();
};

// Helper to build URL with optional namespace
// If namespace is an empty string or null/undefined, the query parameter is omitted.
const buildUrl = (basePath, namespace) => {
    let url = basePath;
    if (namespace && namespace !== "") { // Only add namespace query param if it's provided and not an empty string
        url += `?namespace=${encodeURIComponent(namespace)}`;
    }
    return url;
};

// --- Deployments ---
export const getDeployments = async (clusterId, namespace) => {
    const url = buildUrl(`${API_BASE_URL}/clusters/${clusterId}/deployments`, namespace);
    const response = await fetch(url);
    return handleResponse(response);
};

export const getPodsForDeployment = async (clusterId, namespace, deploymentName) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/namespaces/${namespace}/deployments/${deploymentName}/pods`);
    return handleResponse(response);
};

// --- Services ---
export const getServices = async (clusterId, namespace) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/services?namespace=${namespace}`);
    return handleResponse(response);
};

// --- DaemonSets ---
export const getDaemonSets = async (clusterId, namespace) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/daemonsets?namespace=${namespace}`);
    return handleResponse(response);
};

export const getPodsForDaemonSet = async (clusterId, namespace, daemonSetName) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/namespaces/${namespace}/daemonsets/${daemonSetName}/pods`);
    return handleResponse(response);
};


// --- StatefulSets ---
export const getStatefulSets = async (clusterId, namespace) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/statefulsets?namespace=${namespace}`);
    return handleResponse(response);
};

export const getPodsForStatefulSet = async (clusterId, namespace, statefulSetName) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/namespaces/${namespace}/statefulsets/${statefulSetName}/pods`);
    return handleResponse(response);
};

// --- PersistentVolumeClaims (PVCs) ---
export const getPVCs = async (clusterId, namespace) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/pvcs?namespace=${namespace}`);
    return handleResponse(response);
};

// --- PersistentVolumes (PVs) ---
export const getPVs = async (clusterId) => { // PVs are not namespaced
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/pvs`);
    return handleResponse(response);
};

// --- Nodes ---
export const getNodes = async (clusterId) => { // Nodes are not namespaced
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/nodes`);
    return handleResponse(response);
};

// --- Namespaces --- (Listing all namespaces is not namespaced itself)
export const getNamespaces = async (clusterId) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/namespaces`);
    return handleResponse(response);
};

// --- Pods ---
// General pod listing
export const getPods = async (clusterId, namespace) => {
    const url = buildUrl(`${API_BASE_URL}/clusters/${clusterId}/pods`, namespace);
    const response = await fetch(url);
    return handleResponse(response);
};

export const getPodDetails = async (clusterId, namespace, podName) => {
    const response = await fetch(`${API_BASE_URL}/clusters/${clusterId}/namespaces/${namespace}/pods/${podName}`);
    return handleResponse(response);
};

export const getPodLogs = async (clusterId, namespace, podName, containerName) => {
    let url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${namespace}/pods/${podName}/logs`;
    if (containerName) {
        url += `?container=${containerName}`;
    }
    const response = await fetch(url);
    // Logs are typically plain text
    if (!response.ok) {
        const errorText = await response.text().catch(() => 'Failed to fetch logs and parse error text.');
        console.error('API Error fetching logs:', response.status, errorText);
        throw new Error(errorText || `API request for logs failed with status ${response.status}`);
    }
    return response.text(); 
};


// --- Mock Data (can be removed or kept for fallback/testing) ---
// const MOCK_DELAY = 500; 
// ... (rest of the mock data and functions can be removed or commented out)
// ...
// export const getDeployments = async () => {
// return new Promise(resolve => setTimeout(() => resolve({ data: mockDeployments }), MOCK_DELAY));
// };
// ... (etc. for all mock functions)

// Note: The mock data and associated functions (getDeployments, getServices, etc. that return Promises with mockData)
// have been replaced by the actual fetch calls above.
// You can choose to remove the mock data variables (mockDeployments, mockServices, etc.) and the old mock functions entirely,
// or keep them commented out for reference or testing purposes.
// For this refactoring, I'm assuming they will be replaced.
