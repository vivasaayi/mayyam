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
import { getNodes } from '../../services/kubernetesApiService';

const NodesGrid = ({ clusterId }) => {
    const [nodes, setNodes] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (!clusterId) {
            setNodes([]);
            setLoading(false);
            setError(null);
            return;
        }

        setLoading(true);
        setError(null);
        getNodes(clusterId)
            .then(response => {
                // The backend directly returns the array
                setNodes(Array.isArray(response) ? response : []);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching nodes:", err);
                setError(err.message || 'Failed to fetch nodes');
                setNodes([]);
                setLoading(false);
            });
    }, [clusterId]);

    if (loading) return <p>Loading nodes...</p>;
    if (error) return <p>Error fetching nodes: {error}</p>;
    if (!clusterId) return <p>Please select a cluster to view Nodes.</p>;

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
                            <tr key={node.name}> {/* Node names are unique cluster-wide */}
                                <td style={tableCellStyle}>{node.name}</td>
                                <td style={tableCellStyle}>{node.status || 'N/A'}</td>
                                <td style={tableCellStyle}>{node.roles ? node.roles.join(', ') : 'N/A'}</td>
                                <td style={tableCellStyle}>{node.age || 'N/A'}</td>
                                <td style={tableCellStyle}>{node.kubelet_version || 'N/A'}</td>
                                <td style={tableCellStyle}>{node.internal_ip || 'N/A'}</td>
                                <td style={tableCellStyle}>{node.external_ip || 'N/A'}</td>
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
