import React, { useState, useEffect } from 'react';
import { getPVCs } from '../../services/kubernetesApiService';

const PersistentVolumeClaimsGrid = () => {
    const [pvcs, setPvcs] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        setLoading(true);
        getPVCs()
            .then(response => {
                setPvcs(response.data);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching PVCs:", err);
                setError(err.message || 'Failed to fetch PVCs');
                setLoading(false);
            });
    }, []);

    if (loading) return <p>Loading PVCs...</p>;
    if (error) return <p>Error fetching PVCs: {error}</p>;

    return (
        <div>
            {pvcs.length === 0 ? (
                <p>No PersistentVolumeClaims found.</p>
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
                            <tr key={pvc.id}>
                                <td style={tableCellStyle}>{pvc.name}</td>
                                <td style={tableCellStyle}>{pvc.namespace}</td>
                                <td style={tableCellStyle}>{pvc.status}</td>
                                <td style={tableCellStyle}>{pvc.volume}</td>
                                <td style={tableCellStyle}>{pvc.capacity}</td>
                                <td style={tableCellStyle}>{pvc.accessModes}</td>
                                <td style={tableCellStyle}>{pvc.storageClass}</td>
                                <td style={tableCellStyle}>{pvc.age}</td>
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
