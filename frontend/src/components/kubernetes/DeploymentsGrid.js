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


import React, { useState, useEffect } from 'react';
import { getDeployments } from '../../services/kubernetesApiService';

const DeploymentsGrid = ({ clusterId, namespace, onShowPods }) => { // Added clusterId and namespace props
    const [deployments, setDeployments] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (!clusterId) { // Don't fetch if clusterId is not available
            setLoading(false);
            setDeployments([]); // Clear deployments if no clusterId
            return;
        }

        setLoading(true);
        setError(null); // Reset error state on new fetch
        getDeployments(clusterId, namespace)
            .then(data => { // Assuming data is the array of deployments directly from handleResponse
                setDeployments(data || []); // Ensure data is an array
                setLoading(false);
            })
            .catch(err => {
                console.error(`Error fetching deployments for cluster ${clusterId}, namespace ${namespace}:`, err);
                setError(err.message || 'Failed to fetch deployments');
                setDeployments([]); // Clear deployments on error
                setLoading(false);
            });
    }, [clusterId, namespace]); // Re-fetch when clusterId or namespace changes

    if (loading) return <p>Loading deployments...</p>;
    // Display error specific to the current context if possible
    if (error) return <p>Error fetching deployments {namespace ? `in namespace ${namespace}` : 'for all namespaces'}: {error}</p>;

    return (
        <div>
            {deployments.length === 0 ? (
                <p>No deployments found {namespace ? `in namespace ${namespace}` : 'for all namespaces'}.</p>
            ) : (
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                        <tr>
                            <th style={tableHeaderStyle}>Name</th>
                            <th style={tableHeaderStyle}>Namespace</th>
                            <th style={tableHeaderStyle}>Replicas</th>
                            {/* Assuming backend provides these fields in DeploymentInfo */}
                            <th style={tableHeaderStyle}>Ready</th> 
                            <th style={tableHeaderStyle}>Available</th>
                            <th style={tableHeaderStyle}>Updated</th>
                            <th style={tableHeaderStyle}>Age</th>
                            <th style={tableHeaderStyle}>Images</th>
                            <th style={tableHeaderStyle}>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {deployments.map(dep => (
                            // Assuming backend provides a unique 'name' and 'namespace' combination for key, or an 'id'
                            // Using name + namespace for key if id is not present from backend
                            <tr key={`${dep.namespace}/${dep.name}`}>
                                <td style={tableCellStyle}>{dep.name}</td>
                                <td style={tableCellStyle}>{dep.namespace}</td>
                                <td style={tableCellStyle}>{dep.replicas}</td>
                                <td style={tableCellStyle}>{dep.ready_replicas !== undefined ? dep.ready_replicas : dep.readyReplicas}</td>
                                <td style={tableCellStyle}>{dep.available_replicas !== undefined ? dep.available_replicas : dep.availableReplicas}</td>
                                <td style={tableCellStyle}>{dep.updated_replicas !== undefined ? dep.updated_replicas : dep.updatedReplicas}</td>
                                <td style={tableCellStyle}>{dep.age}</td>
                                <td style={tableCellStyle}>{dep.images ? dep.images.join(', ') : 'N/A'}</td>
                                <td style={tableCellStyle}>
                                    {/* Pass the resource details to onShowPods */}
                                    <button onClick={() => onShowPods({ 
                                        kind: 'Deployment', 
                                        name: dep.name, 
                                        namespace: dep.namespace 
                                        // clusterId is already available in PodsModal via props from KubernetesDashboardPage
                                    })}>
                                        View Pods
                                    </button>
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );
};

const tableHeaderStyle = {
    border: '1px solid #ddd',
    padding: '8px',
    textAlign: 'left',
    backgroundColor: '#f2f2f2'
};

const tableCellStyle = {
    border: '1px solid #ddd',
    padding: '8px',
    textAlign: 'left'
};

export default DeploymentsGrid;
