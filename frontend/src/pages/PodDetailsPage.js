import React, { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { 
  CContainer, 
  CRow, 
  CCol, 
  CCard, 
  CCardBody, 
  CCardHeader,
  CBadge,
  CSpinner,
  CAlert,
  CNav,
  CNavItem,
  CNavLink,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell
} from '@coreui/react';
import { getPodDetails, getPodEvents } from '../services/kubernetesApiService';

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
        return (
            <CContainer fluid className="d-flex justify-content-center align-items-center" style={{ minHeight: '200px' }}>
                <CSpinner color="primary" />
                <span className="ms-2">Loading pod details for {podName}...</span>
            </CContainer>
        );
    }

    if (error) {
        return (
            <CContainer fluid>
                <CAlert color="danger">Error: {error}</CAlert>
            </CContainer>
        );
    }

    if (!podDetails) {
        return (
            <CContainer fluid>
                <CAlert color="warning">No details found for pod {podName}.</CAlert>
            </CContainer>
        );
    }

    // Helper to safely access nested properties using optional chaining fallback
    const get = (obj, path, defaultValue = 'N/A') => {
        try {
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
        } catch {
            return defaultValue;
        }
    };

    const renderOverview = () => (
        <CCard>
            <CCardHeader>
                <h5 className="mb-0">Pod Overview</h5>
            </CCardHeader>
            <CCardBody>
                <CRow>
                    <CCol md={6}>
                        <table className="table table-sm">
                            <tbody>
                                <tr>
                                    <td><strong>Name:</strong></td>
                                    <td>{get(podDetails, 'metadata.name')}</td>
                                </tr>
                                <tr>
                                    <td><strong>Namespace:</strong></td>
                                    <td>{get(podDetails, 'metadata.namespace')}</td>
                                </tr>
                                <tr>
                                    <td><strong>Node:</strong></td>
                                    <td>{get(podDetails, 'spec.nodeName')}</td>
                                </tr>
                                <tr>
                                    <td><strong>Status:</strong></td>
                                    <td>
                                        <CBadge color={get(podDetails, 'status.phase') === 'Running' ? 'success' : 'warning'}>
                                            {get(podDetails, 'status.phase')}
                                        </CBadge>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </CCol>
                    <CCol md={6}>
                        <table className="table table-sm">
                            <tbody>
                                <tr>
                                    <td><strong>Pod IP:</strong></td>
                                    <td>{get(podDetails, 'status.podIP')}</td>
                                </tr>
                                <tr>
                                    <td><strong>Host IP:</strong></td>
                                    <td>{get(podDetails, 'status.hostIP')}</td>
                                </tr>
                                <tr>
                                    <td><strong>QoS Class:</strong></td>
                                    <td>{get(podDetails, 'status.qosClass')}</td>
                                </tr>
                                <tr>
                                    <td><strong>Created:</strong></td>
                                    <td>{get(podDetails, 'metadata.creationTimestamp') ? new Date(get(podDetails, 'metadata.creationTimestamp')).toLocaleString() : 'N/A'}</td>
                                </tr>
                            </tbody>
                        </table>
                    </CCol>
                </CRow>
                {get(podDetails, 'metadata.ownerReferences[0].kind') && (
                    <div className="mt-3">
                        <strong>Controlled By:</strong> {`${get(podDetails, 'metadata.ownerReferences[0].kind')}/${get(podDetails, 'metadata.ownerReferences[0].name')}`}
                    </div>
                )}
                {get(podDetails, 'status.conditions', []).length > 0 && (
                    <div className="mt-4">
                        <h6>Conditions:</h6>
                        <CTable hover responsive>
                            <CTableHead>
                                <CTableRow>
                                    <CTableHeaderCell>Type</CTableHeaderCell>
                                    <CTableHeaderCell>Status</CTableHeaderCell>
                                    <CTableHeaderCell>Last Transition</CTableHeaderCell>
                                    <CTableHeaderCell>Reason</CTableHeaderCell>
                                    <CTableHeaderCell>Message</CTableHeaderCell>
                                </CTableRow>
                            </CTableHead>
                            <CTableBody>
                                {get(podDetails, 'status.conditions').map((condition, index) => (
                                    <CTableRow key={index}>
                                        <CTableDataCell>{get(condition, 'type')}</CTableDataCell>
                                        <CTableDataCell>{get(condition, 'status')}</CTableDataCell>
                                        <CTableDataCell>{get(condition, 'lastTransitionTime') ? new Date(get(condition, 'lastTransitionTime')).toLocaleString() : 'N/A'}</CTableDataCell>
                                        <CTableDataCell>{get(condition, 'reason')}</CTableDataCell>
                                        <CTableDataCell>{get(condition, 'message')}</CTableDataCell>
                                    </CTableRow>
                                ))}
                            </CTableBody>
                        </CTable>
                    </div>
                )}
            </CCardBody>
        </CCard>
    );

    const renderContainers = () => (
        <CCard>
            <CCardHeader>
                <h4 className="mb-0">Containers</h4>
            </CCardHeader>
            <CCardBody>
                {get(podDetails, 'spec.containers', []).map((container, index) => (
                    <CCard key={index} className="mb-3 border">
                        <CCardHeader>
                            <h5 className="mb-0">Container: {get(container, 'name')}</h5>
                        </CCardHeader>
                        <CCardBody>
                            <CRow>
                                <CCol md={6}>
                                    <div className="mb-2"><strong>Image:</strong> {get(container, 'image')}</div>
                                    <div className="mb-2"><strong>Image Pull Policy:</strong> {get(container, 'imagePullPolicy')}</div>
                                    <div className="mb-2"><strong>Ready:</strong> {get(podDetails, `status.containerStatuses[${index}].ready`) ? 'Yes' : 'No'}</div>
                                </CCol>
                                <CCol md={6}>
                                    <div className="mb-2"><strong>Restarts:</strong> {get(podDetails, `status.containerStatuses[${index}].restartCount`)}</div>
                                    <div className="mb-2"><strong>State:</strong> {formatContainerState(get(podDetails, `status.containerStatuses[${index}].state`))}</div>
                                    <div className="mb-2"><strong>Last State:</strong> {formatContainerState(get(podDetails, `status.containerStatuses[${index}].lastState`))}</div>
                                </CCol>
                            </CRow>
                            
                            {get(container, 'ports', []).length > 0 && (
                                <div className="mt-3">
                                    <strong>Ports:</strong>
                                    <ul className="mt-2">
                                        {get(container, 'ports').map((port, pIndex) => (
                                            <li key={pIndex}>{get(port, 'name', '-')}:{port.containerPort}/{get(port, 'protocol', 'TCP')} {get(port, 'hostPort') ? `(Host: ${get(port, 'hostPort')})` : ''}</li>
                                        ))}
                                    </ul>
                                </div>
                            )}
                            
                            {get(container, 'volumeMounts', []).length > 0 && (
                                <div className="mt-3">
                                    <strong>Volume Mounts:</strong>
                                    <CTable hover responsive className="mt-2">
                                        <CTableHead>
                                            <CTableRow>
                                                <CTableHeaderCell>Name</CTableHeaderCell>
                                                <CTableHeaderCell>Mount Path</CTableHeaderCell>
                                                <CTableHeaderCell>Read Only</CTableHeaderCell>
                                                <CTableHeaderCell>SubPath</CTableHeaderCell>
                                            </CTableRow>
                                        </CTableHead>
                                        <CTableBody>
                                            {get(container, 'volumeMounts').map((mount, mIndex) => (
                                                <CTableRow key={mIndex}>
                                                    <CTableDataCell>{get(mount, 'name')}</CTableDataCell>
                                                    <CTableDataCell>{get(mount, 'mountPath')}</CTableDataCell>
                                                    <CTableDataCell>{get(mount, 'readOnly') ? 'Yes' : 'No'}</CTableDataCell>
                                                    <CTableDataCell>{get(mount, 'subPath')}</CTableDataCell>
                                                </CTableRow>
                                            ))}
                                        </CTableBody>
                                    </CTable>
                                </div>
                            )}
                        </CCardBody>
                    </CCard>
                ))}
                
                {get(podDetails, 'spec.initContainers', []).length > 0 && (
                    <div className="mt-4">
                        <h5>Init Containers</h5>
                        {get(podDetails, 'spec.initContainers').map((container, index) => (
                            <CCard key={`init-${index}`} className="mb-3 border">
                                <CCardHeader>
                                    <h5 className="mb-0">Init Container: {get(container, 'name')}</h5>
                                </CCardHeader>
                                <CCardBody>
                                    <div className="mb-2"><strong>Image:</strong> {get(container, 'image')}</div>
                                    <div className="mb-2"><strong>State:</strong> {formatContainerState(get(podDetails, `status.initContainerStatuses[${index}].state`))}</div>
                                </CCardBody>
                            </CCard>
                        ))}
                    </div>
                )}
            </CCardBody>
        </CCard>
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
        <CCard>
            <CCardHeader>
                <h4 className="mb-0">Environment Variables</h4>
            </CCardHeader>
            <CCardBody>
                {get(podDetails, 'spec.containers', []).map((container, index) => (
                    <div key={index} className="mb-4">
                        <h5>For Container: {get(container, 'name')}</h5>
                        {get(container, 'env', []).length > 0 ? (
                            <CTable hover responsive className="mt-3">
                                <CTableHead>
                                    <CTableRow>
                                        <CTableHeaderCell>Name</CTableHeaderCell>
                                        <CTableHeaderCell>Value</CTableHeaderCell>
                                        <CTableHeaderCell>From</CTableHeaderCell>
                                    </CTableRow>
                                </CTableHead>
                                <CTableBody>
                                    {get(container, 'env').map((envVar, eIndex) => (
                                        <CTableRow key={eIndex}>
                                            <CTableDataCell>{get(envVar, 'name')}</CTableDataCell>
                                            <CTableDataCell>{get(envVar, 'value', '-')}</CTableDataCell>
                                            <CTableDataCell>{envVar.valueFrom ? Object.keys(envVar.valueFrom).map(key => `${key}: ${get(envVar.valueFrom[key], 'name') || get(envVar.valueFrom[key], 'secretKeyRef.name') || get(envVar.valueFrom[key], 'configMapKeyRef.name')}`).join(', ') : '-'}</CTableDataCell>
                                        </CTableRow>
                                    ))}
                                </CTableBody>
                            </CTable>
                        ) : <p>No environment variables set for this container.</p>}
                    </div>
                ))}
            </CCardBody>
        </CCard>
    );
    
    const renderVolumes = () => (
        <CCard>
            <CCardHeader>
                <h4 className="mb-0">Volumes</h4>
            </CCardHeader>
            <CCardBody>
                {get(podDetails, 'spec.volumes', []).length > 0 ? (
                    <CTable hover responsive>
                        <CTableHead>
                            <CTableRow>
                                <CTableHeaderCell>Name</CTableHeaderCell>
                                <CTableHeaderCell>Type</CTableHeaderCell>
                                <CTableHeaderCell>Details</CTableHeaderCell>
                            </CTableRow>
                        </CTableHead>
                        <CTableBody>
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
                                    <CTableRow key={index}>
                                        <CTableDataCell>{get(volume, 'name')}</CTableDataCell>
                                        <CTableDataCell>{volumeType || 'N/A'}</CTableDataCell>
                                        <CTableDataCell>{details}</CTableDataCell>
                                    </CTableRow>
                                );
                            })}
                        </CTableBody>
                    </CTable>
                ) : <p>No volumes defined for this pod.</p>}
            </CCardBody>
        </CCard>
    );

    const renderEvents = () => {
        if (eventsLoading) {
            return (
                <CCard>
                    <CCardBody className="text-center">
                        <CSpinner color="primary" className="me-2" />
                        Loading events...
                    </CCardBody>
                </CCard>
            );
        }
        if (eventsError) {
            return (
                <CCard>
                    <CCardBody>
                        <CAlert color="danger">Error loading events: {eventsError}</CAlert>
                    </CCardBody>
                </CCard>
            );
        }
        if (!podEvents || podEvents.length === 0) {
            return (
                <CCard>
                    <CCardBody>
                        <p>No events recorded for this pod.</p>
                    </CCardBody>
                </CCard>
            );
        }

        return (
            <CCard>
                <CCardHeader>
                    <h4 className="mb-0">Events</h4>
                </CCardHeader>
                <CCardBody>
                    <CTable hover responsive>
                        <CTableHead>
                            <CTableRow>
                                <CTableHeaderCell>Last Seen</CTableHeaderCell>
                                <CTableHeaderCell>Type</CTableHeaderCell>
                                <CTableHeaderCell>Reason</CTableHeaderCell>
                                <CTableHeaderCell>Object</CTableHeaderCell>
                                <CTableHeaderCell>Message</CTableHeaderCell>
                            </CTableRow>
                        </CTableHead>
                        <CTableBody>
                            {podEvents.map((event, index) => (
                                <CTableRow key={get(event, 'metadata.uid', index)}>
                                    <CTableDataCell>{get(event, 'lastTimestamp') ? new Date(get(event, 'lastTimestamp')).toLocaleString() : (get(event, 'eventTime') ? new Date(get(event, 'eventTime')).toLocaleString() : 'N/A')}</CTableDataCell>
                                    <CTableDataCell>
                                        <CBadge color={get(event, 'type') === 'Warning' ? 'warning' : 'info'}>
                                            {get(event, 'type')}
                                        </CBadge>
                                    </CTableDataCell>
                                    <CTableDataCell>{get(event, 'reason')}</CTableDataCell>
                                    <CTableDataCell>{`${get(event, 'involvedObject.kind', 'N/A')}/${get(event, 'involvedObject.name', 'N/A')}`}</CTableDataCell>
                                    <CTableDataCell>{get(event, 'message')}</CTableDataCell>
                                </CTableRow>
                            ))}
                        </CTableBody>
                    </CTable>
                </CCardBody>
            </CCard>
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
        <CContainer fluid>
            <CCard className="mb-4">
                <CCardHeader>
                    <h1 className="mb-0">Pod Details</h1>
                </CCardHeader>
                <CCardBody>
                    <CRow>
                        <CCol md={4}>
                            <strong>Name:</strong> {get(podDetails, 'metadata.name')}
                        </CCol>
                        <CCol md={4}>
                            <strong>Namespace:</strong> {get(podDetails, 'metadata.namespace')}
                        </CCol>
                        <CCol md={4}>
                            <strong>Cluster ID:</strong> {clusterId}
                        </CCol>
                    </CRow>
                </CCardBody>
            </CCard>
            
            <CNav variant="tabs" className="mb-3">
                {tabs.map(tabName => (
                    <CNavItem key={tabName}>
                        <CNavLink
                            active={activeTab === tabName}
                            onClick={() => setActiveTab(tabName)}
                            style={{ cursor: 'pointer' }}
                        >
                            {tabName}
                        </CNavLink>
                    </CNavItem>
                ))}
            </CNav>
            
            <div className="tab-content">
                {renderTabContent()}
            </div>
        </CContainer>
    );
};

export default PodDetailsPage;
