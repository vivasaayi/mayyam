// This service will handle all API calls to your backend for Kubernetes data.
import { fetchWithAuth } from './api'; // Import fetchWithAuth

// Base URL for the backend API
const API_BASE_URL = `${process.env.REACT_APP_API_URL || "http://localhost:8010"}/api/kubernetes`;

// Helper function to handle fetch responses
const handleResponse = async (response) => {
    // fetchWithAuth already handles 401 by redirecting to login
    // We still need to check for other non-ok responses
    if (!response.ok) {
        const errorData = await response.json().catch(() => ({ message: 'Network response was not ok and failed to parse error JSON.' }));
        console.error('API Error:', response.status, errorData);
        throw new Error(errorData.message || `API request failed with status ${response.status}`);
    }
    // For 204 No Content, response.json() will fail. Check for it.
    if (response.status === 204) {
        return null; // Or an appropriate representation for no content
    }
    return response.json();
};

// --- Deployments ---
export const getDeployments = async (clusterId, namespace) => {
    let url;
    if (namespace && namespace !== "") {
        url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/deployments`;
    } else {
        url = `${API_BASE_URL}/clusters/${clusterId}/deployments`; // For "All Namespaces"
    }
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

export const getPodsForDeployment = async (clusterId, namespace, deploymentName) => {
    const response = await fetchWithAuth(`${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/deployments/${encodeURIComponent(deploymentName)}/pods`);
    return handleResponse(response);
};

// --- Services ---
export const getServices = async (clusterId, namespace) => {
    let url;
    if (namespace && namespace !== "") {
        url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/services`;
    } else {
        // This will point to an "all services" endpoint. Backend support needed.
        url = `${API_BASE_URL}/clusters/${clusterId}/services`;
    }
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

// --- DaemonSets ---
export const getDaemonSets = async (clusterId, namespace) => {
    let url;
    if (namespace && namespace !== "") {
        url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/daemonsets`;
    } else {
        // This will point to an "all daemonsets" endpoint. Backend support needed.
        url = `${API_BASE_URL}/clusters/${clusterId}/daemonsets`;
    }
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

export const getPodsForDaemonSet = async (clusterId, namespace, daemonSetName) => {
    const response = await fetchWithAuth(`${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/daemonsets/${encodeURIComponent(daemonSetName)}/pods`);
    return handleResponse(response);
};


// --- StatefulSets ---
export const getStatefulSets = async (clusterId, namespace) => {
    let url;
    if (namespace && namespace !== "") {
        url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/statefulsets`;
    } else {
        // This will point to an "all statefulsets" endpoint. Backend support needed.
        url = `${API_BASE_URL}/clusters/${clusterId}/statefulsets`;
    }
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

export const getPodsForStatefulSet = async (clusterId, namespace, statefulSetName) => {
    const response = await fetchWithAuth(`${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/statefulsets/${encodeURIComponent(statefulSetName)}/pods`);
    return handleResponse(response);
};

// --- PersistentVolumeClaims (PVCs) ---
export const getPVCs = async (clusterId, namespace) => {
    let url;
    if (namespace && namespace !== "") {
        url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/persistentvolumeclaims`;
    } else {
        // This will point to an "all PVCs" endpoint. Backend support needed.
        url = `${API_BASE_URL}/clusters/${clusterId}/persistentvolumeclaims`;
    }
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

// --- PersistentVolumes (PVs) ---
export const getPVs = async (clusterId) => { // PVs are not namespaced
    const response = await fetchWithAuth(`${API_BASE_URL}/clusters/${clusterId}/persistentvolumes`);
    return handleResponse(response);
};

// --- Nodes ---
export const getNodes = async (clusterId) => { // Nodes are not namespaced
    const response = await fetchWithAuth(`${API_BASE_URL}/clusters/${clusterId}/nodes`);
    return handleResponse(response);
};

// --- Namespaces --- (Listing all namespaces is not namespaced itself)
export const getNamespaces = async (clusterId) => {
    const response = await fetchWithAuth(`${API_BASE_URL}/clusters/${clusterId}/namespaces`);
    return handleResponse(response);
};

// --- Pods ---
// General pod listing - currently requires a namespace.
// If "all pods in cluster" is needed, backend changes would be required.
export const getPods = async (clusterId, namespace) => {
    if (!namespace || namespace === "") {
        // Or handle this by throwing an error, or returning empty array,
        // as there's no current backend endpoint for "all pods in cluster across all namespaces"
        // without specifying a namespace in the path.
        console.warn('getPods called without a namespace. This is not currently supported for general pod listing across all namespaces.');
        // For now, let\'s assume this won\'t be called with empty namespace by UI logic.
        // If it were, it would need a dedicated backend endpoint like /clusters/{clusterId}/pods
        return Promise.resolve([]); // Return empty array or throw error
    }
    const url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/pods`;
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

// Function to get specific pod details
export const getPodDetails = async (clusterId, namespace, podName) => {
    const url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/pods/${encodeURIComponent(podName)}`;
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

// Function to get events for a specific pod
export const getPodEvents = async (clusterId, namespace, podName) => {
    const url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/pods/${encodeURIComponent(podName)}/events`;
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

export const getPodLogs = async (clusterId, namespace, podName, containerName) => {
    let url = `${API_BASE_URL}/clusters/${clusterId}/namespaces/${encodeURIComponent(namespace)}/pods/${encodeURIComponent(podName)}/logs`;
    if (containerName) {
        url += `?container=${encodeURIComponent(containerName)}`;
    }
    const response = await fetchWithAuth(url);
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
