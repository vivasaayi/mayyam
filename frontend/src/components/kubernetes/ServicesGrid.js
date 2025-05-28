import React, { useState, useEffect } from 'react';
import { getServices } from '../../services/kubernetesApiService';

const ServicesGrid = () => {
    const [services, setServices] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        setLoading(true);
        getServices()
            .then(response => {
                setServices(response.data);
                setLoading(false);
            })
            .catch(err => {
                console.error("Error fetching services:", err);
                setError(err.message || 'Failed to fetch services');
                setLoading(false);
            });
    }, []);

    if (loading) return <p>Loading services...</p>;
    if (error) return <p>Error fetching services: {error}</p>;

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
                                <td style={tableCellStyle}>{svc.ports}</td>
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
