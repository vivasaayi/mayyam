import React, { useState, useEffect } from 'react';
import DeploymentsGrid from '../components/kubernetes/DeploymentsGrid';
import ServicesGrid from '../components/kubernetes/ServicesGrid';
import DaemonSetsGrid from '../components/kubernetes/DaemonSetsGrid';
import StatefulSetsGrid from '../components/kubernetes/StatefulSetsGrid';
import PersistentVolumeClaimsGrid from '../components/kubernetes/PersistentVolumeClaimsGrid';
import PersistentVolumesGrid from '../components/kubernetes/PersistentVolumesGrid';
import NodesGrid from '../components/kubernetes/NodesGrid';
import NamespacesGrid from '../components/kubernetes/NamespacesGrid';
import PodsModal from '../components/kubernetes/PodsModal';
import Tab from '../components/common/Tab';
import { getNamespaces } from '../services/kubernetesApiService';
import { getAllClusters } from '../services/clusterManagementService'; // Changed import from getKubernetesClusters to getAllClusters

const KubernetesDashboardPage = () => {
    const [activeTab, setActiveTab] = useState('Deployments');
    const [showPodsModal, setShowPodsModal] = useState(false);
    const [selectedResourceForPods, setSelectedResourceForPods] = useState(null);

    const [namespaces, setNamespaces] = useState([]);
    const [selectedNamespace, setSelectedNamespace] = useState('');
    const [namespacesLoading, setNamespacesLoading] = useState(false);
    const [namespacesError, setNamespacesError] = useState(null);

    // const clusterId = '1'; // Will be replaced by selectedClusterId
    const [kubernetesClusters, setKubernetesClusters] = useState([]);
    const [selectedClusterId, setSelectedClusterId] = useState('');
    const [clustersLoading, setClustersLoading] = useState(false);
    const [clustersError, setClustersError] = useState(null);


    useEffect(() => {
        const fetchClusters = async () => {
            setClustersLoading(true);
            setClustersError(null);
            try {
                const rawResponse = await getAllClusters('kubernetes'); // Changed to use getAllClusters with type
                // Ensure responseArray is always an array
                const responseArray = Array.isArray(rawResponse) ? rawResponse : [];
                
                setKubernetesClusters(responseArray);

                if (responseArray.length > 0 && responseArray[0] && typeof responseArray[0].id === 'string' && responseArray[0].id) {
                    setSelectedClusterId(responseArray[0].id); // Select the first cluster by default
                } else {
                    setSelectedClusterId(''); // Default to empty string if no valid first cluster
                    if (responseArray.length > 0) { // Log if there was a cluster but it was invalid
                        console.warn("First cluster in response does not have a valid 'id' string. Defaulting selectedClusterId to empty.", responseArray[0]);
                    }
                }
            } catch (error) {
                console.error("Failed to fetch Kubernetes clusters:", error);
                setClustersError(error.message || 'Failed to load Kubernetes clusters.');
                setKubernetesClusters([]);
            }
            setClustersLoading(false);
        };
        fetchClusters();
    }, []);

    useEffect(() => {
        const fetchNamespacesList = async () => {
            if (!selectedClusterId) { // Changed from clusterId to selectedClusterId
                setNamespaces([]);
                setNamespacesError(null); // Clear namespace error if no cluster is selected
                return;
            }

            setNamespacesLoading(true);
            setNamespacesError(null);
            try {
                const response = await getNamespaces(selectedClusterId); // Changed from clusterId to selectedClusterId
                const namespaceList = response.data ? response.data : response;
                setNamespaces(namespaceList.map(ns => ns.name));
            } catch (error) {
                console.error("Failed to fetch namespaces:", error);
                setNamespacesError(error.message || 'Failed to load namespaces.');
                setNamespaces([]);
            }
            setNamespacesLoading(false);
        };

        fetchNamespacesList();
    }, [selectedClusterId]); // Changed dependency from clusterId to selectedClusterId

    const handleShowPods = (resource) => {
        setSelectedResourceForPods(resource);
        setShowPodsModal(true);
    };

    const handleClosePodsModal = () => {
        setShowPodsModal(false);
        setSelectedResourceForPods(null);
    };

    const renderTabContent = () => {
        const namespacedGridProps = {
            clusterId: selectedClusterId, // Changed from clusterId to selectedClusterId
            namespace: selectedNamespace,
        };
        const workloadGridProps = {
            ...namespacedGridProps,
            onShowPods: handleShowPods,
        };

        switch (activeTab) {
            case 'Deployments':
                return <DeploymentsGrid {...workloadGridProps} />;
            case 'Services':
                return <ServicesGrid {...namespacedGridProps} />;
            case 'DaemonSets':
                return <DaemonSetsGrid {...workloadGridProps} />;
            case 'StatefulSets':
                return <StatefulSetsGrid {...workloadGridProps} />;
            case 'PVCs':
                return <PersistentVolumeClaimsGrid {...namespacedGridProps} />;
            case 'PVs':
                return <PersistentVolumesGrid clusterId={selectedClusterId} />; // Changed from clusterId to selectedClusterId
            case 'Nodes':
                return <NodesGrid clusterId={selectedClusterId} />; // Changed from clusterId to selectedClusterId
            case 'Namespaces':
                return <NamespacesGrid clusterId={selectedClusterId} />; // Changed from clusterId to selectedClusterId
            default:
                return <p>Select a resource type</p>;
        }
    };

    const tabs = ['Deployments', 'Services', 'DaemonSets', 'StatefulSets', 'PVCs', 'PVs', 'Nodes', 'Namespaces'];

    return (
        <div style={{ padding: '20px' }}>
            <h1 style={{ marginBottom: '20px' }}>Kubernetes Dashboard</h1>
            
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
                {/* Cluster Selector */}
                <div style={{ marginRight: '20px' }}>
                    <label htmlFor="cluster-select" style={{ marginRight: '10px' }}>Cluster:</label>
                    <select
                        id="cluster-select"
                        value={selectedClusterId}
                        onChange={(e) => setSelectedClusterId(e.target.value)}
                        disabled={clustersLoading || !!clustersError || kubernetesClusters.length === 0}
                        style={{ padding: '8px', minWidth: '200px' }}
                    >
                        {clustersLoading && <option value="" disabled>Loading clusters...</option>}
                        {!clustersLoading && clustersError && <option value="" disabled>Error loading clusters</option>}
                        {!clustersLoading && !clustersError && kubernetesClusters.length === 0 && <option value="" disabled>No clusters found</option>}
                        {!clustersLoading && !clustersError && kubernetesClusters.map(cluster => (
                            <option key={cluster.id} value={cluster.id}>{cluster.name}</option>
                        ))}
                    </select>
                    {clustersError && <span style={{ color: 'red', marginLeft: '10px' }}>{clustersError}</span>}
                </div>

                {/* Namespace Selector */}
                <div>
                    <label htmlFor="namespace-select" style={{ marginRight: '10px' }}>Namespace:</label>
                    <select 
                        id="namespace-select"
                        value={selectedNamespace}
                        onChange={(e) => setSelectedNamespace(e.target.value)}
                        disabled={!selectedClusterId || namespacesLoading || !!namespacesError} // Also disable if no cluster selected
                        style={{ padding: '8px', minWidth: '200px' }}
                    >
                        <option value="">All Namespaces</option>
                        {namespacesLoading && <option value="" disabled>Loading namespaces...</option>}
                        {!namespacesLoading && !namespacesError && namespaces.map(ns => (
                            <option key={ns} value={ns}>{ns}</option>
                        ))}
                        {!namespacesLoading && namespacesError && <option value="" disabled>Error loading namespaces</option>}
                    </select>
                    {namespacesError && <span style={{ color: 'red', marginLeft: '10px' }}>{namespacesError}</span>}
                </div>
            </div>

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
            <div style={{ border: '1px solid #ccc', padding: '20px' }}>
                {renderTabContent()}
            </div>
            {showPodsModal && selectedResourceForPods && (
                <PodsModal
                    clusterId={selectedClusterId} // Changed from clusterId to selectedClusterId
                    resourceName={selectedResourceForPods.name}
                    resourceKind={selectedResourceForPods.kind}
                    namespace={selectedResourceForPods.namespace}
                    onClose={handleClosePodsModal}
                />
            )}
        </div>
    );
};

export default KubernetesDashboardPage;
