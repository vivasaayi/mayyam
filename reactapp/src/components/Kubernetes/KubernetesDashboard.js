import React, { useEffect, useState } from 'react';
import { CContainer, CFormSelect, CTabs, CTabList, CTab, CTabContent, CTabPanel } from '@coreui/react';
import DeploymentsTab from './tabs/DeploymentsTab';
import CronJobsTab from './tabs/CronJobsTab';
import DaemonSetsTab from './tabs/DaemonSetsTab';
import StatefulSetsTab from './tabs/StatefulSetsTab';
import PvcsTab from './tabs/PvcsTab';
import PvsTab from './tabs/PvsTab';
import StorageClassesTab from './tabs/StorageClassesTab';
import axios from 'axios';

const KubernetesDashboard = () => {
  const [activeTab, setActiveTab] = useState('deployments');
  const [namespaces, setNamespaces] = useState([]);
  const [selectedNamespace, setSelectedNamespace] = useState('');

  useEffect(() => {
    fetchNamespaces();
  }, []);

  const fetchNamespaces = async () => {
    try {
      const response = await axios.get('/api/kubernetes/namespaces');
      setNamespaces(response.data);
      setSelectedNamespace(response.data[0]);
    } catch (error) {
      console.error('Error fetching namespaces:', error);
    }
  };

  return (
    <CContainer>
      <CFormSelect value={selectedNamespace} onChange={(e) => setSelectedNamespace(e.target.value)}>
        {namespaces.map((namespace, index) => (
          <option key={index} value={namespace}>
            {namespace}
          </option>
        ))}
      </CFormSelect>
      <CTabs activeItemKey={activeTab} onActiveTabChange={setActiveTab}>
        <CTabList>
          <CTab itemKey="deployments">Deployments</CTab>
          <CTab itemKey="cronJobs">CronJobs</CTab>
          <CTab itemKey="daemonSets">Daemon Sets</CTab>
          <CTab itemKey="statefulSets">Stateful Sets</CTab>
          <CTab itemKey="pvcs">PVCs</CTab>
          <CTab itemKey="pvs">PVs</CTab>
          <CTab itemKey="storageClasses">Storage Classes</CTab>
        </CTabList>
        <CTabContent>
          <CTabPanel className="p-3" itemKey="deployments">
            <DeploymentsTab namespace={selectedNamespace} />
          </CTabPanel>
          <CTabPanel className="p-3" itemKey="cronJobs">
            <CronJobsTab namespace={selectedNamespace} />
          </CTabPanel>
          <CTabPanel className="p-3" itemKey="daemonSets">
            <DaemonSetsTab namespace={selectedNamespace} />
          </CTabPanel>
          <CTabPanel className="p-3" itemKey="statefulSets">
            <StatefulSetsTab namespace={selectedNamespace} />
          </CTabPanel>
          <CTabPanel className="p-3" itemKey="pvcs">
            <PvcsTab namespace={selectedNamespace} />
          </CTabPanel>
          <CTabPanel className="p-3" itemKey="pvs">
            <PvsTab namespace={selectedNamespace} />
          </CTabPanel>
          <CTabPanel className="p-3" itemKey="storageClasses">
            <StorageClassesTab namespace={selectedNamespace} />
          </CTabPanel>
        </CTabContent>
      </CTabs>
    </CContainer>
  );
};

export default KubernetesDashboard;