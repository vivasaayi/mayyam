import React, { useState, useEffect } from 'react';
import { getDaemonSets } from '../../services/kubernetesApiService';

const DaemonSetsGrid = ({ clusterId, namespace, onShowPods }) => { // Added clusterId, namespace props
    const [daemonSets, setDaemonSets] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (!clusterId) { // Don't fetch if clusterId is not available
            setLoading(false);
            setDaemonSets([]);
            return;
        }
        setLoading(true);
        setError(null);
        getDaemonSets(clusterId, namespace) // Pass clusterId and namespace
            .then(data => { // Assuming data is the array directly
                setDaemonSets(data || []);
                setLoading(false);
            })
            .catch(err => {
                console.error(`Error fetching daemonsets for cluster ${clusterId}, namespace ${namespace}:`, err);
                setError(err.message || 'Failed to fetch daemonsets');
                setDaemonSets([]);
                setLoading(false);
            });
    }, [clusterId, namespace]); // Re-fetch when clusterId or namespace changes

    if (loading) return <p>Loading daemonsets...</p>;
    if (error) return <p>Error fetching daemonsets {namespace ? `in namespace ${namespace}` : 'for all namespaces'}: {error}</p>;

    return (
        <div>
            {daemonSets.length === 0 ? (
                <p>No daemonsets found.</p>
            ) : (
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                        <tr>
                            <th style={tableHeaderStyle}>Name</th>
                            <th style={tableHeaderStyle}>Namespace</th>
                            <th style={tableHeaderStyle}>Desired</th>
                            <th style={tableHeaderStyle}>Current</th>
                            <th style={tableHeaderStyle}>Ready</th>
                            <th style={tableHeaderStyle}>Up-to-date</th>
                            <th style={tableHeaderStyle}>Available</th>
                            <th style={tableHeaderStyle}>Node Selector</th>
                            <th style={tableHeaderStyle}>Age</th>
                            <th style={tableHeaderStyle}>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {daemonSets.map(ds => (
                            <tr key={ds.id}>
                                <td style={tableCellStyle}>{ds.name}</td>
                                <td style={tableCellStyle}>{ds.namespace}</td>
                                <td style={tableCellStyle}>{ds.desired}</td>
                                <td style={tableCellStyle}>{ds.current}</td>
                                <td style={tableCellStyle}>{ds.ready}</td>
                                <td style={tableCellStyle}>{ds.upToDate}</td>
                                <td style={tableCellStyle}>{ds.available}</td>
                                <td style={tableCellStyle}>{ds.nodeSelector}</td>
                                <td style={tableCellStyle}>{ds.age}</td>
                                <td style={tableCellStyle}>
                                    <button onClick={() => onShowPods({ 
                                        kind: 'DaemonSet', 
                                        name: ds.name, 
                                        namespace: ds.namespace 
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

export default DaemonSetsGrid;
