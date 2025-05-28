import React, { useState, useEffect } from 'react';
import { getStatefulSets } from '../../services/kubernetesApiService';

const StatefulSetsGrid = ({ onShowPods }) => {
    const [statefulSets, setStatefulSets] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        setLoading(true);
        getStatefulSets()
            .then(response => {
                setStatefulSets(response.data);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching statefulsets:", err);
                setError(err.message || 'Failed to fetch statefulsets');
                setLoading(false);
            });
    }, []);

    if (loading) return <p>Loading statefulsets...</p>;
    if (error) return <p>Error fetching statefulsets: {error}</p>;

    return (
        <div>
            {statefulSets.length === 0 ? (
                <p>No statefulsets found.</p>
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
                            <tr key={sts.id}>
                                <td style={tableCellStyle}>{sts.name}</td>
                                <td style={tableCellStyle}>{sts.namespace}</td>
                                <td style={tableCellStyle}>{sts.ready}</td>
                                <td style={tableCellStyle}>{sts.age}</td>
                                <td style={tableCellStyle}>{sts.containers}</td>
                                <td style={tableCellStyle}>{sts.images}</td>
                                <td style={tableCellStyle}>
                                    <button onClick={() => onShowPods('StatefulSet', sts.name, sts.namespace)}>
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
