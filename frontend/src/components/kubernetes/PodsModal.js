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
import {
    getPodsForDeployment,
    getPodsForDaemonSet,
    getPodsForStatefulSet
} from '../../services/kubernetesApiService';
import { Link } from 'react-router-dom';

const PodsModal = ({ clusterId, resourceName, resourceKind, namespace, onClose }) => { // Added clusterId
    const [pods, setPods] = useState([]);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (clusterId && resourceName && resourceKind && namespace) { // Added clusterId check
            const fetchPodsForResource = async () => {
                setIsLoading(true);
                setError(null);
                let fetchFunction;

                switch (resourceKind.toLowerCase()) {
                    case 'deployment':
                    case 'deployments': // Handle plural form if necessary
                        fetchFunction = getPodsForDeployment;
                        break;
                    case 'daemonset':
                    case 'daemonsets':
                        fetchFunction = getPodsForDaemonSet;
                        break;
                    case 'statefulset':
                    case 'statefulsets':
                        fetchFunction = getPodsForStatefulSet;
                        break;
                    default:
                        setError(`Unsupported resource kind: ${resourceKind}`);
                        setIsLoading(false);
                        setPods([]);
                        return;
                }

                try {
                    // Pass clusterId, namespace, and resourceName to the selected fetch function
                    const data = await fetchFunction(clusterId, namespace, resourceName);
                    setPods(data || []); // Assuming the data is the array of pods directly or an object with a data property
                } catch (err) {
                    console.error(`Error fetching pods for ${resourceKind} ${resourceName}:`, err);
                    setError(err.message || 'Failed to fetch pods.');
                    setPods([]);
                } finally {
                    setIsLoading(false);
                }
            };
            fetchPodsForResource();
        }
    }, [clusterId, resourceName, resourceKind, namespace]); // Added clusterId to dependency array

    // Basic modal styling (can be moved to a CSS file)
    const modalStyle = {
        position: 'fixed',
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
        backgroundColor: 'white',
        padding: '20px',
        zIndex: 1000,
        border: '1px solid #ccc',
        borderRadius: '8px',
        boxShadow: '0 4px 8px rgba(0,0,0,0.1)',
        width: '80%',
        maxWidth: '800px',
        maxHeight: '80vh',
        overflowY: 'auto'
    };

    const overlayStyle = {
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        backgroundColor: 'rgba(0,0,0,0.5)',
        zIndex: 999
    };

    const closeButtonStyle = {
        position: 'absolute',
        top: '10px',
        right: '10px',
        cursor: 'pointer',
        fontSize: '1.5em'
    };

    return (
        <>
            <div style={overlayStyle} onClick={onClose}></div>
            <div style={modalStyle}>
                <span style={closeButtonStyle} onClick={onClose}>&times;</span>
                <h3>Pods for {resourceKind}: {namespace}/{resourceName} (Cluster: {clusterId})</h3>
                {isLoading && <p>Loading pods...</p>}
                {error && <p className="error-message">Error loading pods: {error}</p>}
                {!isLoading && !error && pods.length === 0 && (
                    <p>No pods found for this resource.</p>
                )}
                {!isLoading && !error && pods.length > 0 ? (
                    <div className="table-wrapper"> {/* Added for consistent styling */}
                        <table>
                            <thead>
                                <tr>
                                    <th>Name</th>
                                    <th>Ready</th>
                                    <th>Status</th>
                                    <th>Restarts</th>
                                    <th>Age</th>
                                    <th>IP</th>
                                    <th>Node</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                {pods.map(pod => (
                                    // Assuming pod objects from backend have a unique 'name' or 'uid'
                                    // If your pod objects have a specific ID field like `pod.metadata.uid`, use that.
                                    // For now, using pod.name as key, ensure it's unique within the list.
                                    <tr key={pod.name}> 
                                        <td>{pod.name}</td>
                                        <td>{pod.ready_replicas !== undefined ? `${pod.ready_replicas}/${pod.replicas}` : 'N/A'}</td>
                                        <td>{pod.status}</td>
                                        <td>{pod.restarts !== undefined ? pod.restarts : 'N/A'}</td>
                                        <td>{pod.age}</td>
                                        <td>{pod.pod_ip}</td>
                                        <td>{pod.node_name}</td>
                                        <td>
                                            {/* Ensure the link path matches your routing setup for PodDetailsPage */}
                                            <Link to={`/kubernetes/clusters/${clusterId}/namespaces/${namespace}/pods/${pod.name}`} target="_blank" rel="noopener noreferrer">
                                                View Details
                                            </Link>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                ) : null}
            </div>
        </>
    );
};

export default PodsModal;
