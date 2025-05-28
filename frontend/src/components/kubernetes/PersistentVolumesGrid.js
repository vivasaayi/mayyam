import React, { useState, useEffect } from 'react';
import { getPVs } from '../../services/kubernetesApiService';

const PersistentVolumesGrid = () => {
    const [pvs, setPvs] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        setLoading(true);
        getPVs()
            .then(response => {
                setPvs(response.data);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching PVs:", err);
                setError(err.message || 'Failed to fetch PVs');
                setLoading(false);
            });
    }, []);

    if (loading) return <p>Loading PersistentVolumes...</p>;
    if (error) return <p>Error fetching PersistentVolumes: {error}</p>;

    return (
        <div>
            {pvs.length === 0 ? (
                <p>No PersistentVolumes found.</p>
            ) : (
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                        <tr>
                            <th style={tableHeaderStyle}>Name</th>
                            <th style={tableHeaderStyle}>Capacity</th>
                            <th style={tableHeaderStyle}>Access Modes</th>
                            <th style={tableHeaderStyle}>Reclaim Policy</th>
                            <th style={tableHeaderStyle}>Status</th>
                            <th style={tableHeaderStyle}>Claim</th>
                            <th style={tableHeaderStyle}>Storage Class</th>
                            <th style={tableHeaderStyle}>Reason</th>
                            <th style={tableHeaderStyle}>Age</th>
                        </tr>
                    </thead>
                    <tbody>
                        {pvs.map(pv => (
                            <tr key={pv.id}>
                                <td style={tableCellStyle}>{pv.name}</td>
                                <td style={tableCellStyle}>{pv.capacity}</td>
                                <td style={tableCellStyle}>{pv.accessModes}</td>
                                <td style={tableCellStyle}>{pv.reclaimPolicy}</td>
                                <td style={tableCellStyle}>{pv.status}</td>
                                <td style={tableCellStyle}>{pv.claim}</td>
                                <td style={tableCellStyle}>{pv.storageClass}</td>
                                <td style={tableCellStyle}>{pv.reason}</td>
                                <td style={tableCellStyle}>{pv.age}</td>
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

export default PersistentVolumesGrid;
