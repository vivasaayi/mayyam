import React, { useState, useEffect } from 'react';
import { getNamespaces } from '../../services/kubernetesApiService';

const NamespacesGrid = () => {
    const [namespaces, setNamespaces] = useState([]);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState(null);

    useEffect(() => {
        const loadNamespaces = async () => {
            setIsLoading(true);
            try {
                const data = await getNamespaces();
                setNamespaces(data);
                setError(null);
            } catch (err) {
                setError(err.message);
                setNamespaces([]);
            } finally {
                setIsLoading(false);
            }
        };
        loadNamespaces();
    }, []);

    if (isLoading) {
        return <p>Loading namespaces...</p>;
    }

    if (error) {
        return <p>Error loading namespaces: {error}</p>;
    }

    return (
        <div>
            <h3>Namespaces</h3>
            {namespaces.length > 0 ? (
                <table>
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Status</th>
                            <th>Age</th>
                        </tr>
                    </thead>
                    <tbody>
                        {namespaces.map((ns) => (
                            <tr key={ns.name}>
                                <td>{ns.name}</td>
                                <td>{ns.status}</td>
                                <td>{ns.age}</td>
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

export default NamespacesGrid;
