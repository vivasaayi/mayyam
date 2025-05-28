import React, { useState, useEffect } from 'react';
import { useParams } from 'react-router-dom';
import { getPodDetails } from '../services/kubernetesApiService';
import Tab from '../components/common/Tab'; // Import the shared Tab component

const PodDetailsPage = () => {
    const { namespace, podName } = useParams();
    const [podDetails, setPodDetails] = useState(null);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState(null);
    const [activeTab, setActiveTab] = useState('Overview');

    useEffect(() => {
        const fetchDetails = async () => {
            setIsLoading(true);
            try {
                const { data } = await getPodDetails(namespace, podName);
                setPodDetails(data);
                setError(null);
            } catch (err) {
                setError(err.message);
                setPodDetails(null);
            } finally {
                setIsLoading(false);
            }
        };
        fetchDetails();
    }, [namespace, podName]);

    if (isLoading) {
        return <p>Loading pod details for {namespace}/{podName}...</p>;
    }

    if (error) {
        return <p>Error loading pod details: {error}</p>;
    }

    if (!podDetails) {
        return <p>No details found for pod {namespace}/{podName}.</p>;
    }

    const renderOverview = () => (
        <div>
            <h4>Pod Overview</h4>
            <p><strong>Name:</strong> {podDetails.name}</p>
            <p><strong>Namespace:</strong> {podDetails.namespace}</p>
            <p><strong>Node:</strong> {podDetails.nodeName}</p>
            <p><strong>Status:</strong> {podDetails.status}</p>
            <p><strong>IP:</strong> {podDetails.ip}</p>
            <p><strong>Controlled By:</strong> {podDetails.controlledBy}</p>
            {podDetails.conditions && (
                <>
                    <h5>Conditions:</h5>
                    <ul>
                        {podDetails.conditions.map((condition, index) => (
                            <li key={index}>{condition.type}: {condition.status} (Last Transition: {condition.lastTransitionTime || 'N/A'})</li>
                        ))}
                    </ul>
                </>
            )}
        </div>
    );

    const renderContainers = () => (
        <div>
            <h4>Containers</h4>
            {podDetails.containers && podDetails.containers.length > 0 ? (
                podDetails.containers.map((container, index) => (
                    <div key={index} style={{ marginBottom: '15px', border: '1px solid #eee', padding: '10px' }}>
                        <h5>Container: {container.name}</h5>
                        <p><strong>Image:</strong> {container.image}</p>
                        <p><strong>Ready:</strong> {container.ready ? 'Yes' : 'No'}</p>
                        <p><strong>Restarts:</strong> {container.restarts}</p>
                        <p><strong>State:</strong> {container.state}</p>
                        {container.ports && container.ports.length > 0 && (
                            <>
                                <strong>Ports:</strong>
                                <ul>
                                    {container.ports.map((port, pIndex) => (
                                        <li key={pIndex}>{port.containerPort}/{port.protocol}</li>
                                    ))}
                                </ul>
                            </>
                        )}
                        {container.mounts && container.mounts.length > 0 && (
                            <>
                                <strong>Volume Mounts:</strong>
                                <ul>
                                    {container.mounts.map((mount, mIndex) => (
                                        <li key={mIndex}>{mount.name} at {mount.mountPath} ({mount.readOnly ? 'ro' : 'rw'})</li>
                                    ))}
                                </ul>
                            </>
                        )}
                    </div>
                ))
            ) : <p>No container information available.</p>}
        </div>
    );

    const renderEnvironmentVariables = () => (
        <div>
            <h4>Environment Variables</h4>
            {podDetails.containers && podDetails.containers.map((container, index) => (
                <div key={index} style={{ marginBottom: '15px' }}>
                    <h5>For Container: {container.name}</h5>
                    {container.env && container.env.length > 0 ? (
                        <table>
                            <thead>
                                <tr>
                                    <th>Name</th>
                                    <th>Value</th>
                                </tr>
                            </thead>
                            <tbody>
                                {container.env.map((envVar, eIndex) => (
                                    <tr key={eIndex}>
                                        <td>{envVar.name}</td>
                                        <td>{envVar.value || (envVar.valueFrom ? `From ${Object.keys(envVar.valueFrom)[0]}` : 'N/A')}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    ) : <p>No environment variables set for this container.</p>}
                </div>
            ))}
        </div>
    );
    
    const renderVolumes = () => (
        <div>
            <h4>Volumes</h4>
            {podDetails.volumes && podDetails.volumes.length > 0 ? (
                 <table>
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Type</th>
                            <th>Details</th>
                        </tr>
                    </thead>
                    <tbody>
                        {podDetails.volumes.map((volume, index) => (
                            <tr key={index}>
                                <td>{volume.name}</td>
                                <td>{volume.type}</td>
                                <td>
                                    {volume.configMapName && `ConfigMap: ${volume.configMapName}`}
                                    {volume.secretName && `Secret: ${volume.secretName}`}
                                    {/* Add more volume type details as needed */}
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            ) : <p>No volumes defined for this pod.</p>}
        </div>
    );

    const renderEvents = () => (
        <div>
            <h4>Events</h4>
            {podDetails.events && podDetails.events.length > 0 ? (
                <table>
                    <thead>
                        <tr>
                            <th>Type</th>
                            <th>Reason</th>
                            <th>Age</th>
                            <th>From</th>
                            <th>Message</th>
                        </tr>
                    </thead>
                    <tbody>
                        {podDetails.events.map((event, index) => (
                            <tr key={index}>
                                <td>{event.type}</td>
                                <td>{event.reason}</td>
                                <td>{event.age}</td>
                                <td>{event.from}</td>
                                <td>{event.message}</td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            ) : <p>No events recorded for this pod.</p>}
        </div>
    );


    const tabs = ['Overview', 'Containers', 'Environment Variables', 'Volumes', 'Events'];

    const renderTabContent = () => {
        switch (activeTab) {
            case 'Overview':
                return renderOverview();
            case 'Containers':
                return renderContainers();
            case 'Environment Variables':
                return renderEnvironmentVariables();
            case 'Volumes':
                return renderVolumes();
            case 'Events':
                return renderEvents();
            default:
                return renderOverview();
        }
    };

    return (
        <div style={{ padding: '20px' }}>
            <h2 style={{ marginBottom: '10px' }}>Pod: {podDetails.name}</h2>
            <p style={{ marginBottom: '20px' }}>Namespace: {podDetails.namespace}</p>
            
            <div style={{ marginBottom: '20px' }}>
                {tabs.map(tab => (
                    <Tab
                        key={tab}
                        label={tab}
                        isActive={activeTab === tab}
                        onClick={() => setActiveTab(tab)}
                    />
                ))}
            </div>
            <div style={{ border: '1px solid #ccc', padding: '20px', marginTop: '-1px' }}>
                {renderTabContent()}
            </div>
        </div>
    );
};

export default PodDetailsPage;
