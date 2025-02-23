import React, { useState, useEffect } from 'react';
import { CContainer, CTabs, CTabContent, CTabPane, CNav, CNavItem, CNavLink, CButton, CForm, CFormLabel, CFormInput, CTabList, CTabPanel, CTab } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule } from 'ag-grid-community';
import axios from 'axios';

const KubernetesPodDetails = () => {
  const [activeTab, setActiveTab] = useState(0);
  const [podName, setPodName] = useState('');
  const [namespace, setNamespace] = useState('');
  const [podDetails, setPodDetails] = useState(null);

  useEffect(() => {
    const urlParams = new URLSearchParams(window.location.hash.split('?')[1]);
    const podNameParam = urlParams.get('podName');
    const namespaceParam = urlParams.get('namespace');
    if (podNameParam) setPodName(podNameParam);
    if (namespaceParam) setNamespace(namespaceParam);
    // Fetch pod details based on query params
    fetchPodDetails(podNameParam, namespaceParam);
  }, []);

  const fetchPodDetails = async (podName, namespace) => {
    try {
      const response = await axios.get(`/api/kubernetes/pod-details?podName=${podName}&namespace=${namespace}`);
      setPodDetails(response.data);
    } catch (error) {
      console.error('Error fetching pod details:', error);
    }
  };

  const handleReload = () => {
    fetchPodDetails(podName, namespace);
  };

  const renderParsedPodDetails = () => {
    if (!podDetails) return null;
    return (
      <div>
        {/* Render parsed pod details here */}
        <CTabs activeItemKey={activeTab} onActiveTabChange={setActiveTab}>
          <CTabList variant="tabs">
            {podDetails.containers.map((container, index) => (
              <CTab key={index} itemKey={index}>
                {container.name}
              </CTab>
            ))}
          </CTabList>
          <CTabContent>
            {podDetails.containers.map((container, index) => (
              <CTabPanel key={index} className="p-3" itemKey={index}>
                <div className="ag-theme-alpine" style={{ height: 200, width: '100%' }}>
                  <h5>Env Vars</h5>
                  <AgGridReact
                    rowData={container.envVars}
                    columnDefs={[
                      { headerName: 'Name', field: 'name' },
                      { headerName: 'Value', field: 'value' },
                    ]}
                    modules={[ClientSideRowModelModule]}
                  />
                </div>
                <div className="ag-theme-alpine" style={{ height: 200, width: '100%' }}>
                  <h5>Attached Volumes</h5>
                  <AgGridReact
                    rowData={container.volumes}
                    columnDefs={[
                      { headerName: 'Name', field: 'name' },
                      { headerName: 'Mount Path', field: 'mountPath' },
                    ]}
                    modules={[ClientSideRowModelModule]}
                  />
                </div>
              </CTabPanel>
            ))}
          </CTabContent>
        </CTabs>
      </div>
    );
  };

  return (
    <CContainer>
      <CForm>
        <CFormLabel htmlFor="podName">Pod Name</CFormLabel>
        <CFormInput id="podName" value={podName} onChange={(e) => setPodName(e.target.value)} />
        <CFormLabel htmlFor="namespace">Namespace</CFormLabel>
        <CFormInput id="namespace" value={namespace} onChange={(e) => setNamespace(e.target.value)} />
        <CButton onClick={handleReload}>Reload</CButton>
      </CForm>
      <CTabs activeItemKey={activeTab} onActiveTabChange={setActiveTab}>
        <CTabList variant="tabs">
          <CTab itemKey={0}>Parsed Pod Details</CTab>
          <CTab itemKey={1}>Raw JSON</CTab>
        </CTabList>
        <CTabContent>
          <CTabPanel className="p-3" itemKey={0}>{renderParsedPodDetails()}</CTabPanel>
          <CTabPanel className="p-3" itemKey={1}>
            <pre>{JSON.stringify(podDetails, null, 2)}</pre>
          </CTabPanel>
        </CTabContent>
      </CTabs>
    </CContainer>
  );
};

export default KubernetesPodDetails;