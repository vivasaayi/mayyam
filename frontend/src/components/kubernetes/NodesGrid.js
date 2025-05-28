import React, { useState, useEffect } from 'react';
import { getNodes } from '../../services/kubernetesApiService';

const NodesGrid = () => {
    const [nodes, setNodes] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        setLoading(true);
        getNodes()
            .then(response => {
                setNodes(response.data);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching nodes:", err);
                setError(err.message || 'Failed to fetch nodes');
                setLoading(false);
            });
    }, []);

    if (loading) return <p>Loading nodes...</p>;
    if (error) return <p>Error fetching nodes: {error}</p>;

    return (
        <div>
            {nodes.length === 0 ? (
                <p>No nodes found.</p>
            ) : (
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                        <tr>
                            <th style={tableHeaderStyle}>Name</th>
                            <th style={tableHeaderStyle}>Status</th>
                            <th style={tableHeaderStyle}>Roles</th>
                            <th style={tableHeaderStyle}>Age</th>
                            <th style={tableHeaderStyle}>Version</th>
                            <th style={tableHeaderStyle}>Internal IP</th>
                            <th style={tableHeaderStyle}>External IP</th>
                            {/* Consider adding OS Image, Kernel, Container Runtime if needed */}
                        </tr>
                    </thead>
                    <tbody>
                        {nodes.map(node => (
                            <tr key={node.id}>
                                <td style={tableCellStyle}>{node.name}</td>
                                <td style={tableCellStyle}>{node.status}</td>
                                <td style={tableCellStyle}>{node.roles}</td>
                                <td style={tableCellStyle}>{node.age}</td>
                                <td style={tableCellStyle}>{node.version}</td>
                                <td style={tableCellStyle}>{node.internalIP}</td>
                                <td style={tableCellStyle}>{node.externalIP}</td>
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

export default NodesGrid;
