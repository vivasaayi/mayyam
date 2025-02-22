import React, { useState, useEffect } from 'react';
import { CContainer, CTabs, CTabContent, CTabPane, CNav, CNavItem, CNavLink, CButton, CForm, CFormGroup, CLabel, CInput } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import axios from 'axios';

const KubernetesPodDetails = () => {
  const [activeTab, setActiveTab] = useState(0);
  const [podName, setPodName] = useState('');
  const [namespace, setNamespace] = useState('');
  const [podDetails, setPodDetails] = useState(null);

  useEffect(() => {
    const urlParams = new URLSearchParams(window.location.search);
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
        <CTabs>
          <CNav variant="tabs">
            {podDetails.containers.map((container, index) => (
              <CNavItem key={index}>
                <CNavLink active={activeTab === index} onClick={() => setActiveTab(index)}>
                  {container.name}
                </CNavLink>
              </CNavItem>
            ))}
          </CNav>
          <CTabContent>
            {podDetails.containers.map((container, index) => (
              <CTabPane key={index} active={activeTab === index}>
                <div className="ag-theme-alpine" style={{ height: 200, width: '100%' }}>
                  <h5>Env Vars</h5>
                  <AgGridReact
                    rowData={container.envVars}
                    columnDefs={[
                      { headerName: 'Name', field: 'name' },
                      { headerName: 'Value', field: 'value' },
                    ]}
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
                  />
                </div>
              </CTabPane>
            ))}
          </CTabContent>
        </CTabs>
      </div>
    );
  };

  return (
    <CContainer>
      <CForm>
        <CFormGroup>
          <CLabel htmlFor="podName">Pod Name</CLabel>
          <CInput id="podName" value={podName} onChange={(e) => setPodName(e.target.value)} />
        </CFormGroup>
        <CFormGroup>
          <CLabel htmlFor="namespace">Namespace</CLabel>
          <CInput id="namespace" value={namespace} onChange={(e) => setNamespace(e.target.value)} />
        </CFormGroup>
        <CButton onClick={handleReload}>Reload</CButton>
      </CForm>
      <CTabs>
        <CNav variant="tabs">
          <CNavItem>
            <CNavLink active={activeTab === 0} onClick={() => setActiveTab(0)}>
              Parsed Pod Details
            </CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink active={activeTab === 1} onClick={() => setActiveTab(1)}>
              Raw JSON
            </CNavLink>
          </CNavItem>
        </CNav>
        <CTabContent>
          <CTabPane active={activeTab === 0}>{renderParsedPodDetails()}</CTabPane>
          <CTabPane active={activeTab === 1}>
            <pre>{JSON.stringify(podDetails, null, 2)}</pre>
          </CTabPane>
        </CTabContent>
      </CTabs>
    </CContainer>
  );
};

export default KubernetesPodDetails;