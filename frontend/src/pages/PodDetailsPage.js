import React, { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { getPodDetails, getPodEvents } from '../services/kubernetesApiService'; // Added getPodEvents
import Tab from '../components/common/Tab';

const PodDetailsPage = () => {
    const { clusterId, namespace, podName } = useParams();
    const [podDetails, setPodDetails] = useState(null);
    const [podEvents, setPodEvents] = useState([]); // State for pod events
    const [loading, setLoading] = useState(true);
    const [eventsLoading, setEventsLoading] = useState(true); // Separate loading state for events
    const [error, setError] = useState(null);
    const [eventsError, setEventsError] = useState(null); // Separate error state for events
    const [activeTab, setActiveTab] = useState('Overview');

    useEffect(() => {
        const fetchDetails = async () => {
            if (!clusterId || !namespace || !podName) {
                setError("Cluster ID, namespace, or pod name is missing from URL.");
                setLoading(false);
                setEventsLoading(false); // Also set events loading to false
                return;
            }
            setLoading(true);
            setError(null);
            try {
                const details = await getPodDetails(clusterId, namespace, podName);
                setPodDetails(details);
            } catch (err) {
                console.error(`Error fetching pod details for ${podName} in ${namespace} on cluster ${clusterId}:`, err);
                setError(err.message || "Failed to fetch pod details.");
            }
            setLoading(false);
        };

        const fetchEvents = async () => {
            if (!clusterId || !namespace || !podName) {
                // Error already handled by fetchDetails
                setEventsLoading(false);
                return;
            }
            setEventsLoading(true);
            setEventsError(null);
            try {
                const events = await getPodEvents(clusterId, namespace, podName);
                // Sort events by last timestamp, most recent first
                const sortedEvents = events.sort((a, b) => {
                    const timeA = a?.lastTimestamp || a?.eventTime || a?.metadata?.creationTimestamp;
                    const timeB = b?.lastTimestamp || b?.eventTime || b?.metadata?.creationTimestamp;
                    if (!timeA && !timeB) return 0;
                    if (!timeA) return 1; // Put events without time at the end
                    if (!timeB) return -1; // Put events without time at the end
                    return new Date(timeB) - new Date(timeA);
                });
                setPodEvents(sortedEvents || []);
            } catch (err) {
                console.error(`Error fetching events for pod ${podName} in ${namespace} on cluster ${clusterId}:`, err);
                setEventsError(err.message || "Failed to fetch pod events.");
            }
            setEventsLoading(false);
        };

        fetchDetails();
        fetchEvents(); // Call fetchEvents
    }, [clusterId, namespace, podName]);

    if (loading) {
        return <p>Loading pod details for {podName}...</p>;
    }

    if (error) {
        return <p style={{ color: 'red' }}>Error: {error}</p>;
    }

    if (!podDetails) {
        return <p>No details found for pod {podName}.</p>;
    }

    // Helper to safely access nested properties
    const get = (obj, path, defaultValue = 'N/A') => {
        const keys = Array.isArray(path) ? path : path.split('.');
        let result = obj;
        for (const key of keys) {
            if (result && typeof result === 'object' && key in result) {
                result = result[key];
            } else {
                return defaultValue;
            }
        }
        return result === null || result === undefined ? defaultValue : result;
    };

    const renderOverview = () => (
        <div>
            <h4>Pod Overview</h4>
            <p><strong>Name:</strong> {get(podDetails, 'metadata.name')}</p>
            <p><strong>Namespace:</strong> {get(podDetails, 'metadata.namespace')}</p>
            <p><strong>Node:</strong> {get(podDetails, 'spec.nodeName')}</p>
            <p><strong>Status:</strong> {get(podDetails, 'status.phase')}</p>
            <p><strong>Pod IP:</strong> {get(podDetails, 'status.podIP')}</p>
            <p><strong>Host IP:</strong> {get(podDetails, 'status.hostIP')}</p>
            <p><strong>QoS Class:</strong> {get(podDetails, 'status.qosClass')}</p>
            <p><strong>Created:</strong> {get(podDetails, 'metadata.creationTimestamp') ? new Date(get(podDetails, 'metadata.creationTimestamp')).toLocaleString() : 'N/A'}</p>
            {get(podDetails, 'metadata.ownerReferences[0].kind') && 
                <p><strong>Controlled By:</strong> {`${get(podDetails, 'metadata.ownerReferences[0].kind')}/${get(podDetails, 'metadata.ownerReferences[0].name')}`}</p>
            }
            {get(podDetails, 'status.conditions', []).length > 0 && (
                <>
                    <h5>Conditions:</h5>
                    <table style={tableStyle}>
                        <thead><tr><th style={thStyle}>Type</th><th style={thStyle}>Status</th><th style={thStyle}>Last Transition</th><th style={thStyle}>Reason</th><th style={thStyle}>Message</th></tr></thead>
                        <tbody>
                            {get(podDetails, 'status.conditions').map((condition, index) => (
                                <tr key={index}>
                                    <td style={tdStyle}>{get(condition, 'type')}</td>
                                    <td style={tdStyle}>{get(condition, 'status')}</td>
                                    <td style={tdStyle}>{get(condition, 'lastTransitionTime') ? new Date(get(condition, 'lastTransitionTime')).toLocaleString() : 'N/A'}</td>
                                    <td style={tdStyle}>{get(condition, 'reason')}</td>
                                    <td style={tdStyle}>{get(condition, 'message')}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </>
            )}
        </div>
    );

    const renderContainers = () => (
        <div>
            <h4>Containers</h4>
            {get(podDetails, 'spec.containers', []).map((container, index) => (
                <div key={index} style={{ marginBottom: '15px', border: '1px solid #eee', padding: '10px' }}>
                    <h5>Container: {get(container, 'name')}</h5>
                    <p><strong>Image:</strong> {get(container, 'image')}</p>
                    <p><strong>Image Pull Policy:</strong> {get(container, 'imagePullPolicy')}</p>
                    <p><strong>Ready:</strong> {get(podDetails, `status.containerStatuses[${index}].ready`) ? 'Yes' : 'No'}</p>
                    <p><strong>Restarts:</strong> {get(podDetails, `status.containerStatuses[${index}].restartCount`)}</p>
                    <p><strong>State:</strong> {formatContainerState(get(podDetails, `status.containerStatuses[${index}].state`))}</p>
                    <p><strong>Last State:</strong> {formatContainerState(get(podDetails, `status.containerStatuses[${index}].lastState`))}</p>
                    
                    {get(container, 'ports', []).length > 0 && (
                        <>
                            <strong>Ports:</strong>
                            <ul>
                                {get(container, 'ports').map((port, pIndex) => (
                                    <li key={pIndex}>{get(port, 'name', '-')}:{port.containerPort}/{get(port, 'protocol', 'TCP')} {get(port, 'hostPort') ? `(Host: ${get(port, 'hostPort')})` : ''}</li>
                                ))}
                            </ul>
                        </>
                    )}
                    {get(container, 'volumeMounts', []).length > 0 && (
                        <>
                            <strong>Volume Mounts:</strong>
                            <table style={tableStyle}>
                                <thead><tr><th style={thStyle}>Name</th><th style={thStyle}>Mount Path</th><th style={thStyle}>Read Only</th><th style={thStyle}>SubPath</th></tr></thead>
                                <tbody>
                                {get(container, 'volumeMounts').map((mount, mIndex) => (
                                    <tr key={mIndex}>
                                        <td style={tdStyle}>{get(mount, 'name')}</td>
                                        <td style={tdStyle}>{get(mount, 'mountPath')}</td>
                                        <td style={tdStyle}>{get(mount, 'readOnly') ? 'Yes' : 'No'}</td>
                                        <td style={tdStyle}>{get(mount, 'subPath')}</td>
                                    </tr>
                                ))}
                                </tbody>
                            </table>
                        </>
                    )}
                    {/* Add Env Vars, Probes etc. here if needed */}
                </div>
            ))}
            {get(podDetails, 'spec.initContainers', []).length > 0 && (
                <>
                    <h5 style={{marginTop: '20px'}}>Init Containers</h5>
                    {get(podDetails, 'spec.initContainers').map((container, index) => (
                         <div key={`init-${index}`} style={{ marginBottom: '15px', border: '1px solid #eee', padding: '10px' }}>
                            <h5>Init Container: {get(container, 'name')}</h5>
                            <p><strong>Image:</strong> {get(container, 'image')}</p>
                            <p><strong>State:</strong> {formatContainerState(get(podDetails, `status.initContainerStatuses[${index}].state`))}</p>
                         </div>
                    ))}
                </>
            )}
        </div>
    );

    const formatContainerState = (state) => {
        if (!state) return 'N/A';
        const stateKey = Object.keys(state)[0];
        if (!stateKey) return 'N/A';
        const details = state[stateKey];
        let str = stateKey.charAt(0).toUpperCase() + stateKey.slice(1);
        if (details.reason) str += ` (${details.reason})`;
        if (details.message) str += `: ${details.message}`;
        if (details.startedAt) str += ` (Started: ${new Date(details.startedAt).toLocaleString()})`;
        if (details.finishedAt) str += ` (Finished: ${new Date(details.finishedAt).toLocaleString()})`;
        return str;
    };

    const renderEnvironmentVariables = () => (
        <div>
            <h4>Environment Variables</h4>
            {get(podDetails, 'spec.containers', []).map((container, index) => (
                <div key={index} style={{ marginBottom: '15px' }}>
                    <h5>For Container: {get(container, 'name')}</h5>
                    {get(container, 'env', []).length > 0 ? (
                        <table style={tableStyle}>
                            <thead><tr><th style={thStyle}>Name</th><th style={thStyle}>Value</th><th style={thStyle}>From</th></tr></thead>
                            <tbody>
                                {get(container, 'env').map((envVar, eIndex) => (
                                    <tr key={eIndex}>
                                        <td style={tdStyle}>{get(envVar, 'name')}</td>
                                        <td style={tdStyle}>{get(envVar, 'value', '-')}</td>
                                        <td style={tdStyle}>{envVar.valueFrom ? Object.keys(envVar.valueFrom).map(key => `${key}: ${get(envVar.valueFrom[key], 'name') || get(envVar.valueFrom[key], 'secretKeyRef.name') || get(envVar.valueFrom[key], 'configMapKeyRef.name')}`).join(', ') : '-'}</td>
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
            {get(podDetails, 'spec.volumes', []).length > 0 ? (
                 <table style={tableStyle}>
                    <thead>
                        <tr>
                            <th style={thStyle}>Name</th>
                            <th style={thStyle}>Type</th>
                            <th style={thStyle}>Details</th>
                        </tr>
                    </thead>
                    <tbody>
                        {get(podDetails, 'spec.volumes').map((volume, index) => {
                            const volumeType = Object.keys(volume).find(key => key !== 'name');
                            let details = '-';
                            if (volumeType && volume[volumeType]) {
                                if (typeof volume[volumeType] === 'object') {
                                    details = Object.entries(volume[volumeType]).map(([k,v]) => `${k}: ${v}`).join(', ');
                                } else {
                                    details = String(volume[volumeType]);
                                }
                            }
                            return (
                                <tr key={index}>
                                    <td style={tdStyle}>{get(volume, 'name')}</td>
                                    <td style={tdStyle}>{volumeType || 'N/A'}</td>
                                    <td style={tdStyle}>{details}</td>
                                </tr>
                            );
                        })}
                    </tbody>
                </table>
            ) : <p>No volumes defined for this pod.</p>}
        </div>
    );

    // Placeholder for Events - this would typically require a separate API call in Kubernetes
    // as events are not directly part of the Pod object in the same way other details are.
    // For now, we'll leave it as a placeholder.
    const renderEvents = () => {
        if (eventsLoading) {
            return <p>Loading events...</p>;
        }
        if (eventsError) {
            return <p style={{ color: 'red' }}>Error loading events: {eventsError}</p>;
        }
        if (!podEvents || podEvents.length === 0) {
            return <p>No events recorded for this pod.</p>;
        }

        return (
            <div>
                <h4>Events</h4>
                <table style={tableStyle}>
                    <thead>
                        <tr>
                            <th style={thStyle}>Last Seen</th>
                            <th style={thStyle}>Type</th>
                            <th style={thStyle}>Reason</th>
                            <th style={thStyle}>Object</th>
                            <th style={thStyle}>Message</th>
                        </tr>
                    </thead>
                    <tbody>
                        {podEvents.map((event, index) => (
                            <tr key={get(event, 'metadata.uid', index)}> {/* Use event UID as key if available */}
                                <td style={tdStyle}>{get(event, 'lastTimestamp') ? new Date(get(event, 'lastTimestamp')).toLocaleString() : (get(event, 'eventTime') ? new Date(get(event, 'eventTime')).toLocaleString() : 'N/A')}</td>
                                <td style={tdStyle}>{get(event, 'type')}</td>
                                <td style={tdStyle}>{get(event, 'reason')}</td>
                                <td style={tdStyle}>{`${get(event, 'involvedObject.kind', 'N/A')}/${get(event, 'involvedObject.name', 'N/A')}`}</td>
                                <td style={tdStyle}>{get(event, 'message')}</td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
        );
    };

    const tabs = ['Overview', 'Containers', 'Environment Variables', 'Volumes', 'Events'];

    const renderTabContent = () => {
        switch (activeTab) {
            case 'Overview': return renderOverview();
            case 'Containers': return renderContainers();
            case 'Environment Variables': return renderEnvironmentVariables();
            case 'Volumes': return renderVolumes();
            case 'Events': return renderEvents();
            default: return renderOverview();
        }
    };

    return (
        <div style={{ padding: '20px' }}>
            <h1 style={{ marginBottom: '20px' }}>Pod Details</h1>
            <div style={{ marginBottom: '10px' }}>
                <strong>Name:</strong> {get(podDetails, 'metadata.name')}<br />
                <strong>Namespace:</strong> {get(podDetails, 'metadata.namespace')}<br />
                <strong>Cluster ID:</strong> {clusterId} 
            </div>
            
            <div style={{ marginBottom: '20px' }}>
                {tabs.map(tabName => (
                    <Tab
                        key={tabName}
                        label={tabName}
                        isActive={activeTab === tabName}
                        onClick={() => setActiveTab(tabName)}
                    />
                ))}
            </div>
            <div style={{ border: '1px solid #ccc', padding: '20px', marginTop: '-1px' }}>
                {renderTabContent()}
            </div>
        </div>
    );
};

const tableStyle = { width: '100%', borderCollapse: 'collapse', marginTop: '10px' };
const thStyle = { border: '1px solid #ddd', padding: '8px', textAlign: 'left', backgroundColor: '#f2f2f2' };
const tdStyle = { border: '1px solid #ddd', padding: '8px', textAlign: 'left' };

export default PodDetailsPage;
