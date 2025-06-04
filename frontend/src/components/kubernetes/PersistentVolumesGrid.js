import React, { useState, useEffect } from 'react';
import { getPVs } from '../../services/kubernetesApiService';

const PersistentVolumesGrid = ({ clusterId }) => {
    const [pvs, setPvs] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        if (!clusterId) {
            setPvs([]);
            setLoading(false);
            setError(null);
            return;
        }

        setLoading(true);
        setError(null);
        getPVs(clusterId)
            .then(response => {
                 // The backend directly returns the array
                setPvs(Array.isArray(response) ? response : []);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching PVs:", err);
                setError(err.message || 'Failed to fetch PVs');
                setPvs([]);
                setLoading(false);
            });
    }, [clusterId]);

    if (loading) return <p>Loading PersistentVolumes...</p>;
    if (error) return <p>Error fetching PersistentVolumes: {error}</p>;
    if (!clusterId) return <p>Please select a cluster to view Persistent Volumes.</p>;

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
                            <tr key={pv.name}> {/* PV names are unique cluster-wide */}
                                <td style={tableCellStyle}>{pv.name}</td>
                                <td style={tableCellStyle}>{pv.capacity || 'N/A'}</td>
                                <td style={tableCellStyle}>{pv.access_modes ? pv.access_modes.join(', ') : 'N/A'}</td>
                                <td style={tableCellStyle}>{pv.reclaim_policy || 'N/A'}</td>
                                <td style={tableCellStyle}>{pv.status || 'N/A'}</td>
                                <td style={tableCellStyle}>{pv.claim_ref || 'N/A'}</td>
                                <td style={tableCellStyle}>{pv.storage_class_name || 'N/A'}</td>
                                <td style={tableCellStyle}>{pv.reason || 'N/A'}</td>
                                <td style={tableCellStyle}>{pv.age || 'N/A'}</td>
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
