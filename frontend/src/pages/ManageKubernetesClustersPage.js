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


import React, { useState, useEffect, useCallback } from 'react';
import * as clusterService from '../services/clusterManagementService'; // Import the new service

const ManageKubernetesClustersPage = () => {
  const [clusters, setClusters] = useState([]);
  const [newClusterName, setNewClusterName] = useState('');
  const [newClusterApiServerUrl, setNewClusterApiServerUrl] = useState('');
  const [newClusterToken, setNewClusterToken] = useState(''); // For K8s token
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState(null);

  // --- Editing State ---
  const [editingCluster, setEditingCluster] = useState(null); // Cluster object being edited
  const [editClusterName, setEditClusterName] = useState('');
  const [editClusterApiServerUrl, setEditClusterApiServerUrl] = useState('');
  const [editClusterToken, setEditClusterToken] = useState('');

  const fetchClusters = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      // Fetch only kubernetes clusters for this page
      const data = await clusterService.getAllClusters('kubernetes');
      setClusters(data || []); // Ensure data is an array
    } catch (err) {
      console.error("Error fetching clusters:", err);
      setError(err.message || 'Failed to fetch clusters.');
      setClusters([]); // Set to empty array on error
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchClusters();
  }, [fetchClusters]);

  const handleAddCluster = async (e) => {
    e.preventDefault();
    if (!newClusterName.trim() || !newClusterApiServerUrl.trim()) {
      alert('Please provide a name and API Server URL for the cluster.');
      return;
    }
    setIsLoading(true);
    setError(null);
    const clusterData = {
      name: newClusterName,
      api_server_url: newClusterApiServerUrl,
      token: newClusterToken || null, // Send null if empty, backend Option<String>
      // Add other fields like kube_config_path, kube_context if you plan to support them in the form
      kube_config_path: null,
      kube_context: null,
      certificate_authority_data: null,
      client_certificate_data: null,
      client_key_data: null,
    };
    try {
      await clusterService.createKubernetesCluster(clusterData);
      setNewClusterName('');
      setNewClusterApiServerUrl('');
      setNewClusterToken('');
      fetchClusters(); // Refresh the list
    } catch (err) {
      console.error("Error adding cluster:", err);
      setError(err.message || 'Failed to add cluster.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleDeleteCluster = async (clusterId) => {
    if (!window.confirm('Are you sure you want to delete this cluster?')) {
      return;
    }
    setIsLoading(true);
    setError(null);
    try {
      await clusterService.deleteCluster(clusterId);
      fetchClusters(); // Refresh the list
    } catch (err) {
      console.error("Error deleting cluster:", err);
      setError(err.message || 'Failed to delete cluster.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleEditClick = (cluster) => {
    setEditingCluster(cluster);
    setEditClusterName(cluster.name);
    setEditClusterApiServerUrl(cluster.config?.api_server_url || '');
    setEditClusterToken(cluster.config?.token || '');
  };

  const handleCancelEdit = () => {
    setEditingCluster(null);
    setEditClusterName('');
    setEditClusterApiServerUrl('');
    setEditClusterToken('');
  };

  const handleUpdateCluster = async (e) => {
    e.preventDefault();
    if (!editingCluster || !editClusterName.trim() || !editClusterApiServerUrl.trim()) {
      alert('Please provide a name and API Server URL for the cluster update.');
      return;
    }
    setIsLoading(true);
    setError(null);
    const updatedData = {
      name: editClusterName,
      api_server_url: editClusterApiServerUrl,
      token: editClusterToken || null,
      // Preserve other config fields if they exist and are not being edited
      // This depends on how your backend handles updates (full vs partial config update)
      // The current backend `update` in ClusterRepository replaces the whole `config` JSON blob.
      // So, we should provide all expected fields for KubernetesClusterConfig.
      kube_config_path: editingCluster.config?.kube_config_path || null,
      kube_context: editingCluster.config?.kube_context || null,
      certificate_authority_data: editingCluster.config?.certificate_authority_data || null,
      client_certificate_data: editingCluster.config?.client_certificate_data || null,
      client_key_data: editingCluster.config?.client_key_data || null,
    };

    try {
      await clusterService.updateKubernetesCluster(editingCluster.id, updatedData);
      setEditingCluster(null);
      fetchClusters(); // Refresh list
    } catch (err) {
      console.error("Error updating cluster:", err);
      setError(err.message || 'Failed to update cluster.');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div style={{ padding: '20px' }}>
      <h2>Manage Kubernetes Clusters</h2>

      {isLoading && <p>Loading...</p>}
      {error && <p style={{ color: 'red' }}>Error: {error}</p>}

      {!editingCluster ? (
        <form onSubmit={handleAddCluster} style={{ marginBottom: '30px', padding: '15px', border: '1px solid #eee', borderRadius: '5px' }}>
          <h3>Add New Kubernetes Cluster</h3>
          <div style={{ marginBottom: '10px' }}>
            <label htmlFor="clusterName" style={{ marginRight: '10px', display: 'block' }}>Cluster Name:</label>
            <input
              type="text"
              id="clusterName"
              value={newClusterName}
              onChange={(e) => setNewClusterName(e.target.value)}
              placeholder="e.g., my-dev-cluster"
              required
              style={{ width: '300px', padding: '8px' }}
            />
          </div>
          <div style={{ marginBottom: '10px' }}>
            <label htmlFor="clusterApiServerUrl" style={{ marginRight: '10px', display: 'block' }}>Cluster API Server URL:</label>
            <input
              type="text"
              id="clusterApiServerUrl"
              value={newClusterApiServerUrl}
              onChange={(e) => setNewClusterApiServerUrl(e.target.value)}
              placeholder="e.g., https://your-kube-api-server"
              required
              style={{ width: '400px', padding: '8px' }}
            />
          </div>
          <div style={{ marginBottom: '10px' }}>
            <label htmlFor="clusterToken" style={{ marginRight: '10px', display: 'block' }}>Bearer Token (Optional):</label>
            <input
              type="text"
              id="clusterToken"
              value={newClusterToken}
              onChange={(e) => setNewClusterToken(e.target.value)}
              placeholder="Kubernetes bearer token"
              style={{ width: '400px', padding: '8px' }}
            />
          </div>
          <button type="submit" style={{ marginTop: '10px', padding: '10px 15px' }} disabled={isLoading}>
            {isLoading ? 'Adding...' : 'Add Cluster'}
          </button>
        </form>
      ) : (
        <form onSubmit={handleUpdateCluster} style={{ marginBottom: '30px', padding: '15px', border: '1px solid #eee', borderRadius: '5px' }}>
          <h3>Edit Kubernetes Cluster: {editingCluster.name}</h3>
          <div style={{ marginBottom: '10px' }}>
            <label htmlFor="editClusterName" style={{ marginRight: '10px', display: 'block' }}>Cluster Name:</label>
            <input
              type="text"
              id="editClusterName"
              value={editClusterName}
              onChange={(e) => setEditClusterName(e.target.value)}
              required
              style={{ width: '300px', padding: '8px' }}
            />
          </div>
          <div style={{ marginBottom: '10px' }}>
            <label htmlFor="editClusterApiServerUrl" style={{ marginRight: '10px', display: 'block' }}>Cluster API Server URL:</label>
            <input
              type="text"
              id="editClusterApiServerUrl"
              value={editClusterApiServerUrl}
              onChange={(e) => setEditClusterApiServerUrl(e.target.value)}
              required
              style={{ width: '400px', padding: '8px' }}
            />
          </div>
          <div style={{ marginBottom: '10px' }}>
            <label htmlFor="editClusterToken" style={{ marginRight: '10px', display: 'block' }}>Bearer Token (Optional):</label>
            <input
              type="text"
              id="editClusterToken"
              value={editClusterToken}
              onChange={(e) => setEditClusterToken(e.target.value)}
              style={{ width: '400px', padding: '8px' }}
            />
          </div>
          <button type="submit" style={{ marginTop: '10px', padding: '10px 15px', marginRight: '10px' }} disabled={isLoading}>
            {isLoading ? 'Updating...' : 'Update Cluster'}
          </button>
          <button type="button" onClick={handleCancelEdit} style={{ padding: '10px 15px' }} disabled={isLoading}>
            Cancel
          </button>
        </form>
      )}

      <h3>Registered Kubernetes Clusters</h3>
      {clusters.length === 0 && !isLoading ? (
        <p>No Kubernetes clusters configured yet.</p>
      ) : (
        <ul style={{ listStyleType: 'none', padding: 0 }}>
          {clusters.map(cluster => (
            <li key={cluster.id} style={{ marginBottom: '10px', padding: '10px', border: '1px solid #ccc', borderRadius: '4px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <strong>Name:</strong> {cluster.name} <br />
                {/* Display API server URL and token from the config object */}
                <strong>API Server URL:</strong> {cluster.config?.api_server_url || 'N/A'} <br />
                <strong>Token:</strong> {cluster.config?.token ? '******' : 'Not set'} <br />
                <small>ID: {cluster.id}</small><br/>
                <small>Type: {cluster.cluster_type}</small>
              </div>
              <div>
                <button onClick={() => handleEditClick(cluster)} style={{ marginRight: '5px' }} disabled={isLoading || !!editingCluster}>
                  Edit
                </button>
                <button onClick={() => handleDeleteCluster(cluster.id)} disabled={isLoading || !!editingCluster}>
                  Delete
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

export default ManageKubernetesClustersPage;
