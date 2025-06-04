import { fetchWithAuth } from './api'; // Assuming api.js exports fetchWithAuth

const API_BASE_URL = process.env.REACT_APP_API_URL || "http://localhost:8080";

// Helper function to handle responses (similar to kubernetesApiService)
const handleResponse = async (response) => {
    if (!response.ok) {
        // fetchWithAuth already handles 401 by redirecting to login
        const errorData = await response.json().catch(() => ({ 
            message: response.statusText || 'Network response was not ok and failed to parse error JSON.' 
        }));
        console.error('Cluster Management API Error:', response.status, errorData);
        throw new Error(errorData.message || `API request failed with status ${response.status}`);
    }
    if (response.status === 204) { // No Content
        return null;
    }
    return response.json();
};

/**
 * Fetches all clusters, optionally filtered by type.
 * @param {string} [clusterType] - Optional. The type of cluster to filter by (e.g., "kubernetes").
 * @returns {Promise<Array<object>>} A promise that resolves to an array of cluster objects.
 */
export const getAllClusters = async (clusterType) => {
    // If clusterType is specifically 'kubernetes', use the new endpoint
    // Otherwise, this function might need to be split or adapted if it was meant to be generic
    if (clusterType === 'kubernetes') {
        const url = `${API_BASE_URL}/api/kubernetes-clusters`; // Changed endpoint
        const response = await fetchWithAuth(url);
        return handleResponse(response);
    }
    // Fallback or error for other types if this service is only for k8s now
    // For now, let's assume we only call this with 'kubernetes' from ManageKubernetesClustersPage
    console.warn(`getAllClusters called with unhandled type: ${clusterType}`);
    // To maintain previous behavior for other types (if any were used), you'd need the old /api/clusters endpoint
    // or specific functions for other cluster types e.g., getAllKafkaClusters
    let url = `${API_BASE_URL}/api/clusters`; // This would be the old generic one
    if (clusterType) {
        url += `?type=${encodeURIComponent(clusterType)}`;
    }
    const response = await fetchWithAuth(url);
    return handleResponse(response); 
    // Or throw an error:
    // throw new Error(`Cluster type '${clusterType}' is not supported by this function for k8s specific service.`);
};

/**
 * Fetches a specific cluster by its ID.
 * @param {string} clusterId - The ID of the cluster to fetch.
 * @returns {Promise<object>} A promise that resolves to the cluster object.
 */
export const getClusterById = async (clusterId) => {
    // This now specifically gets a Kubernetes cluster by ID
    const url = `${API_BASE_URL}/api/kubernetes-clusters/${clusterId}`; // Changed endpoint
    const response = await fetchWithAuth(url);
    return handleResponse(response);
};

/**
 * Creates a new Kubernetes cluster.
 * @param {object} clusterData - The data for the new Kubernetes cluster.
 * Expected fields: name, api_server_url, and other fields from CreateKubernetesClusterRequest model.
 * e.g., { name: "my-cluster", api_server_url: "https://localhost:6443", token: "..." }
 * @returns {Promise<object>} A promise that resolves to the created cluster object.
 */
export const createKubernetesCluster = async (clusterData) => {
    const url = `${API_BASE_URL}/api/kubernetes-clusters`; // Changed endpoint, removed /kubernetes suffix
    const response = await fetchWithAuth(url, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(clusterData),
    });
    return handleResponse(response);
};

/**
 * Updates an existing Kubernetes cluster.
 * @param {string} clusterId - The ID of the cluster to update.
 * @param {object} clusterData - The updated data for the Kubernetes cluster.
 * Expected fields: name, api_server_url, etc., from UpdateKubernetesClusterRequest model.
 * @returns {Promise<object>} A promise that resolves to the updated cluster object.
 */
export const updateKubernetesCluster = async (clusterId, clusterData) => {
    const url = `${API_BASE_URL}/api/kubernetes-clusters/${clusterId}`; // Changed endpoint
    const response = await fetchWithAuth(url, {
        method: 'PUT',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(clusterData),
    });
    return handleResponse(response);
};

/**
 * Deletes a cluster by its ID.
 * @param {string} clusterId - The ID of the cluster to delete.
 * @returns {Promise<null>} A promise that resolves when the cluster is deleted.
 */
export const deleteCluster = async (clusterId) => {
    // This now specifically deletes a Kubernetes cluster by ID
    const url = `${API_BASE_URL}/api/kubernetes-clusters/${clusterId}`; // Changed endpoint
    const response = await fetchWithAuth(url, {
        method: 'DELETE',
    });
    return handleResponse(response); // Expects 204 No Content on success
};
