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
import { getPVCs } from '../../services/kubernetesApiService';

const PersistentVolumeClaimsGrid = ({ clusterId, namespace }) => {
    const [pvcs, setPvcs] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (!clusterId) {
            setPvcs([]);
            setLoading(false);
            setError(null);
            return;
        }

        setLoading(true);
        setError(null);
        getPVCs(clusterId, namespace)
            .then(response => {
                setPvcs(Array.isArray(response) ? response : []);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching PVCs:", err);
                setError(err.message || 'Failed to fetch PVCs');
                setPvcs([]);
                setLoading(false);
            });
    }, [clusterId, namespace]);

    if (loading) return <p>Loading PVCs...</p>;
    if (error) return <p>Error fetching PVCs: {error}</p>;
    if (!clusterId) return <p>Please select a cluster to view PVCs.</p>;

    return (
        <div>
            {pvcs.length === 0 ? (
                <p>No PersistentVolumeClaims found{namespace ? ` in namespace "${namespace}"` : ""}.</p>
            ) : (
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                        <tr>
                            <th style={tableHeaderStyle}>Name</th>
                            <th style={tableHeaderStyle}>Namespace</th>
                            <th style={tableHeaderStyle}>Status</th>
                            <th style={tableHeaderStyle}>Volume</th>
                            <th style={tableHeaderStyle}>Capacity</th>
                            <th style={tableHeaderStyle}>Access Modes</th>
                            <th style={tableHeaderStyle}>Storage Class</th>
                            <th style={tableHeaderStyle}>Age</th>
                        </tr>
                    </thead>
                    <tbody>
                        {pvcs.map(pvc => (
                            <tr key={`${pvc.namespace}-${pvc.name}`}>
                                <td style={tableCellStyle}>{pvc.name}</td>
                                <td style={tableCellStyle}>{pvc.namespace}</td>
                                <td style={tableCellStyle}>{pvc.status || 'N/A'}</td>
                                <td style={tableCellStyle}>{pvc.volume_name || 'N/A'}</td>
                                <td style={tableCellStyle}>{pvc.capacity || 'N/A'}</td>
                                <td style={tableCellStyle}>{pvc.access_modes ? pvc.access_modes.join(', ') : 'N/A'}</td>
                                <td style={tableCellStyle}>{pvc.storage_class_name || 'N/A'}</td>
                                <td style={tableCellStyle}>{pvc.age || 'N/A'}</td>
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

export default PersistentVolumeClaimsGrid;
