import React, { useState, useEffect } from 'react';
import { getStatefulSets } from '../../services/kubernetesApiService';

const StatefulSetsGrid = ({ clusterId, namespace, onShowPods }) => {
    const [statefulSets, setStatefulSets] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (!clusterId) {
            setStatefulSets([]);
            setLoading(false);
            setError(null); // Clear error if no clusterId
            return;
        }

        setLoading(true);
        setError(null);
        getStatefulSets(clusterId, namespace)
            .then(response => {
                // The backend directly returns the array of statefulsets
                setStatefulSets(Array.isArray(response) ? response : []);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching statefulsets:", err);
                setError(err.message || 'Failed to fetch statefulsets');
                setStatefulSets([]); // Clear data on error
                setLoading(false);
            });
    }, [clusterId, namespace]);

    if (loading) return <p>Loading statefulsets...</p>;
    if (error) return <p>Error fetching statefulsets: {error}</p>;
    if (!clusterId) return <p>Please select a cluster to view StatefulSets.</p>;

    return (
        <div>
            {statefulSets.length === 0 ? (
                <p>No statefulsets found{namespace ? ` in namespace "${namespace}"` : ""}.</p>
            ) : (
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                        <tr>
                            <th style={tableHeaderStyle}>Name</th>
                            <th style={tableHeaderStyle}>Namespace</th>
                            <th style={tableHeaderStyle}>Ready</th>
                            <th style={tableHeaderStyle}>Age</th>
                            <th style={tableHeaderStyle}>Containers</th>
                            <th style={tableHeaderStyle}>Images</th>
                            <th style={tableHeaderStyle}>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {statefulSets.map(sts => (
                            <tr key={`${sts.namespace}-${sts.name}`}> {/* Ensure unique key if names can repeat across namespaces */}
                                <td style={tableCellStyle}>{sts.name}</td>
                                <td style={tableCellStyle}>{sts.namespace}</td>
                                <td style={tableCellStyle}>{sts.ready_replicas !== undefined && sts.replicas !== undefined ? `${sts.ready_replicas}/${sts.replicas}` : 'N/A'}</td>
                                <td style={tableCellStyle}>{sts.age || 'N/A'}</td>
                                <td style={tableCellStyle}>{sts.containers ? sts.containers.join(', ') : 'N/A'}</td>
                                <td style={tableCellStyle}>{sts.images ? sts.images.join(', ') : 'N/A'}</td>
                                <td style={tableCellStyle}>
                                    <button onClick={() => onShowPods({ // Pass the whole sts object or necessary parts
                                        kind: 'StatefulSet', 
                                        name: sts.name, 
                                        namespace: sts.namespace 
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

export default StatefulSetsGrid;
