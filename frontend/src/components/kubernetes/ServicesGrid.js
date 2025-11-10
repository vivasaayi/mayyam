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
import { getServices } from '../../services/kubernetesApiService';

const ServicesGrid = ({ clusterId, namespace }) => { // Added clusterId, namespace props
    const [services, setServices] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (!clusterId) { // Don't fetch if clusterId is not available
            setLoading(false);
            setServices([]);
            return;
        }
        setLoading(true);
        setError(null);
        getServices(clusterId, namespace) // Pass clusterId and namespace
            .then(data => { // Assuming data is the array directly
                setServices(data || []);
                setLoading(false);
            })
            .catch(err => {
                console.error(`Error fetching services for cluster ${clusterId}, namespace ${namespace}:`, err);
                setError(err.message || 'Failed to fetch services');
                setServices([]);
                setLoading(false);
            });
    }, [clusterId, namespace]); // Re-fetch when clusterId or namespace changes

    if (loading) return <p>Loading services...</p>;
    if (error) return <p>Error fetching services {namespace ? `in namespace ${namespace}` : 'for all namespaces'}: {error}</p>;

    return (
        <div>
            {services.length === 0 ? (
                <p>No services found.</p>
            ) : (
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                        <tr>
                            <th style={tableHeaderStyle}>Name</th>
                            <th style={tableHeaderStyle}>Namespace</th>
                            <th style={tableHeaderStyle}>Type</th>
                            <th style={tableHeaderStyle}>Cluster IP</th>
                            <th style={tableHeaderStyle}>External IP</th>
                            <th style={tableHeaderStyle}>Ports</th>
                            <th style={tableHeaderStyle}>Age</th>
                            {/* Add other relevant headers if needed */}
                        </tr>
                    </thead>
                    <tbody>
                        {services.map(svc => (
                            <tr key={svc.id}>
                                <td style={tableCellStyle}>{svc.name}</td>
                                <td style={tableCellStyle}>{svc.namespace}</td>
                                <td style={tableCellStyle}>{svc.type}</td>
                                <td style={tableCellStyle}>{svc.clusterIP}</td>
                                <td style={tableCellStyle}>{svc.externalIP}</td>
                                <td style={tableCellStyle}>
                                    {
                                        Array.isArray(svc.ports) && svc.ports.length > 0
                                        ? svc.ports.map(p => {
                                            let portStr = '';
                                            if (p.name) portStr += `${p.name}:`;
                                            portStr += `${p.port}/${p.protocol}`;
                                            if (p.target_port) portStr += ` -> ${p.target_port}`;
                                            if (p.node_port) portStr += ` (Node: ${p.node_port})`;
                                            return portStr;
                                        }).join(', ')
                                        : 'N/A'
                                    }
                                </td>
                                <td style={tableCellStyle}>{svc.age}</td>
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

export default ServicesGrid;
