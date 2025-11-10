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
import { getNamespaces } from '../../services/kubernetesApiService';

const NamespacesGrid = ({ clusterId }) => {
    const [namespaces, setNamespaces] = useState([]);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (!clusterId) {
            setNamespaces([]);
            setIsLoading(false);
            setError(null);
            return;
        }

        const loadNamespaces = async () => {
            setIsLoading(true);
            setError(null);
            try {
                // getNamespaces in the service expects clusterId
                const response = await getNamespaces(clusterId);
                // Assuming the response is directly the array of namespaces
                setNamespaces(Array.isArray(response) ? response : (response.data && Array.isArray(response.data) ? response.data : []));
                
            } catch (err) {
                console.error("Error fetching namespaces:", err);
                setError(err.message || 'Failed to load namespaces');
                setNamespaces([]);
            } finally {
                setIsLoading(false);
            }
        };
        loadNamespaces();
    }, [clusterId]);

    if (isLoading) {
        return <p>Loading namespaces...</p>;
    }

    if (error) {
        return <p>Error loading namespaces: {error}</p>;
    }
    if (!clusterId) return <p>Please select a cluster to view Namespaces.</p>;

    return (
        <div>
            <h3>Namespaces</h3>
            {namespaces.length > 0 ? (
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                        <tr>
                            <th style={tableHeaderStyle}>Name</th>
                            <th style={tableHeaderStyle}>Status</th>
                            <th style={tableHeaderStyle}>Age</th>
                        </tr>
                    </thead>
                    <tbody>
                        {namespaces.map((ns) => (
                            <tr key={ns.name}> {/* Namespace names are unique cluster-wide */}
                                <td style={tableCellStyle}>{ns.name}</td>
                                <td style={tableCellStyle}>{ns.status || 'N/A'}</td>
                                <td style={tableCellStyle}>{ns.age || 'N/A'}</td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            ) : (
                <p>No namespaces found.</p>
            )}
        </div>
    );
};

// Add table styles if not already present from other components, or import them
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

export default NamespacesGrid;
