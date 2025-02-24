import React, { useState, useEffect } from 'react';
import { CContainer, CTabs, CTabContent, CTabPane, CNav, CNavItem, CNavLink, CButton, CForm, CFormLabel, CFormInput, CTabList, CTabPanel, CTab } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule, DateFilterModule, ModuleRegistry, NumberFilterModule, TextFilterModule, ValidationModule } from 'ag-grid-community';
import axios from 'axios';

ModuleRegistry.registerModules([ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]);

const KubernetesPodDetails = () => {
  const [activeTab, setActiveTab] = useState(0);
  const [podName, setPodName] = useState('');
  const [namespace, setNamespace] = useState('');
  const [podDetails, setPodDetails] = useState(null);
  const [podStatus, setPodStatus] = useState(null);
  const [podEvents, setPodEvents] = useState([]);
  const [error, setError] = useState(null);

  useEffect(() => {
    const urlParams = new URLSearchParams(window.location.hash.split('?')[1]);
    const podNameParam = urlParams.get('podName');
    const namespaceParam = urlParams.get('namespace');
    if (podNameParam) setPodName(podNameParam);
    if (namespaceParam) setNamespace(namespaceParam);
    // Fetch pod details, status, and events based on query params
    fetchPodDetails(podNameParam, namespaceParam);
    fetchPodStatus(podNameParam, namespaceParam);
    fetchPodEvents(podNameParam, namespaceParam);
  }, []);

  const fetchPodDetails = async (podName, namespace) => {
    try {
      const response = await axios.get(`/api/kubernetes/pod-details?podName=${podName}&namespace=${namespace}`);
      setPodDetails(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching pod details:', error);
      setError('Error fetching pod details');
    }
  };

  const fetchPodStatus = async (podName, namespace) => {
    try {
      const response = await axios.get(`/api/kubernetes/pod-status?podName=${podName}&namespace=${namespace}`);
      setPodStatus(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching pod status:', error);
      setError('Error fetching pod status');
    }
  };

  const fetchPodEvents = async (podName, namespace) => {
    try {
      const response = await axios.get(`/api/kubernetes/pod-events?podName=${podName}&namespace=${namespace}`);
      setPodEvents(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching pod events:', error);
      setError('Error fetching pod events');
    }
  };

  const handleReload = () => {
    fetchPodDetails(podName, namespace);
    fetchPodStatus(podName, namespace);
    fetchPodEvents(podName, namespace);
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
                    defaultColDef={{ flex: 1, minWidth: 100, sortable: true, filter: true, resizable: true }}
                    pagination={true}
                    paginationPageSize={10}
                    sideBar={{ toolPanels: ['columns', 'filters'] }}
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
                    defaultColDef={{ flex: 1, minWidth: 100, sortable: true, filter: true, resizable: true }}
                    pagination={true}
                    paginationPageSize={10}
                    sideBar={{ toolPanels: ['columns', 'filters'] }}
                  />
                </div>
              </CTabPanel>
            ))}
          </CTabContent>
        </CTabs>
      </div>
    );
  };

  const renderPodStatus = () => {
    if (!podStatus) return null;
    return (
      <div className="ag-theme-alpine" style={{ height: 200, width: '100%' }}>
        <h5>Pod Status</h5>
        <AgGridReact
          rowData={[podStatus]}
          columnDefs={[
            { headerName: 'Phase', field: 'phase' },
            { 
              headerName: 'Conditions', 
              field: 'conditions', 
              valueFormatter: (params) => JSON.stringify(params.value) 
            },
          ]}
          modules={[ClientSideRowModelModule]}
          defaultColDef={{ flex: 1, minWidth: 100, sortable: true, filter: true, resizable: true }}
          pagination={true}
          paginationPageSize={10}
          sideBar={{ toolPanels: ['columns', 'filters'] }}
        />
      </div>
    );
  };

  const renderPodEvents = () => {
    if (!podEvents.length) return null;
    return (
      <div className="ag-theme-alpine" style={{ height: 200, width: '100%' }}>
        <h5>Pod Events</h5>
        <AgGridReact
          rowData={podEvents}
          columnDefs={[
            { headerName: 'Type', field: 'type' },
            { headerName: 'Reason', field: 'reason' },
            { headerName: 'Message', field: 'message' },
            { headerName: 'First Timestamp', field: 'firstTimestamp' },
            { headerName: 'Last Timestamp', field: 'lastTimestamp' },
          ]}
          modules={[ClientSideRowModelModule]}
          defaultColDef={{ flex: 1, minWidth: 100, sortable: true, filter: true, resizable: true }}
          pagination={true}
          paginationPageSize={10}
          sideBar={{ toolPanels: ['columns', 'filters'] }}
        />
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
      {error && <div className="alert alert-danger">{error}</div>}
      <CTabs activeItemKey={activeTab} onActiveTabChange={setActiveTab}>
        <CTabList variant="tabs">
          <CTab itemKey={0}>Parsed Pod Details</CTab>
          <CTab itemKey={1}>Pod Status</CTab>
          <CTab itemKey={2}>Pod Events</CTab>
          <CTab itemKey={3}>Raw JSON</CTab>
        </CTabList>
        <CTabContent>
          <CTabPanel className="p-3" itemKey={0}>{renderParsedPodDetails()}</CTabPanel>
          <CTabPanel className="p-3" itemKey={1}>{renderPodStatus()}</CTabPanel>
          <CTabPanel className="p-3" itemKey={2}>{renderPodEvents()}</CTabPanel>
          <CTabPanel className="p-3" itemKey={3}>
            <pre>{JSON.stringify(podDetails, null, 2)}</pre>
          </CTabPanel>
        </CTabContent>
      </CTabs>
    </CContainer>
  );
};

export default KubernetesPodDetails;