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

const KubernetesDashboardPage = () => {
    const [activeTab, setActiveTab] = useState('Deployments');
    const [showPodsModal, setShowPodsModal] = useState(false);
    const [selectedResourceForPods, setSelectedResourceForPods] = useState(null);

    const [namespaces, setNamespaces] = useState([]);
    const [selectedNamespace, setSelectedNamespace] = useState('');
    const [namespacesLoading, setNamespacesLoading] = useState(false);
    const [namespacesError, setNamespacesError] = useState(null);

    const clusterId = '1';

    useEffect(() => {
        const fetchNamespacesList = async () => {
            if (!clusterId) return;

            setNamespacesLoading(true);
            setNamespacesError(null);
            try {
                const response = await getNamespaces(clusterId);
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
    }, [clusterId]);

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
            clusterId,
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
                return <PersistentVolumesGrid clusterId={clusterId} />;
            case 'Nodes':
                return <NodesGrid clusterId={clusterId} />;
            case 'Namespaces':
                return <NamespacesGrid clusterId={clusterId} />;
            default:
                return <p>Select a resource type</p>;
        }
    };

    const tabs = ['Deployments', 'Services', 'DaemonSets', 'StatefulSets', 'PVCs', 'PVs', 'Nodes', 'Namespaces'];

    return (
        <div style={{ padding: '20px' }}>
            <h1 style={{ marginBottom: '20px' }}>Kubernetes Dashboard</h1>
            
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
                {/* Namespace Selector */}
                <div>
                    <label htmlFor="namespace-select" style={{ marginRight: '10px' }}>Namespace:</label>
                    <select 
                        id="namespace-select"
                        value={selectedNamespace}
                        onChange={(e) => setSelectedNamespace(e.target.value)}
                        disabled={namespacesLoading || !!namespacesError}
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
                    clusterId={clusterId}
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
