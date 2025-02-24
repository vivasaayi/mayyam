import React, { useEffect, useState } from 'react';
import { CContainer, CButton, CFormSelect } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule, DateFilterModule, ModuleRegistry, NumberFilterModule, TextFilterModule, ValidationModule } from 'ag-grid-community';
import axios from 'axios';

ModuleRegistry.registerModules([ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]);

const KubernetesPods = () => {
  const [rowData, setRowData] = useState([]);
  const [error, setError] = useState(null);
  const [namespaces, setNamespaces] = useState([]);
  const [selectedNamespace, setSelectedNamespace] = useState('');

  useEffect(() => {
    fetchNamespaces();
  }, []);

  useEffect(() => {
    if (selectedNamespace) {
      fetchData(selectedNamespace);
    }
  }, [selectedNamespace]);

  const fetchNamespaces = async () => {
    try {
      const response = await axios.get('/api/kubernetes/namespaces');
      setNamespaces(response.data);
      setSelectedNamespace(response.data[0]);
    } catch (error) {
      console.error('Error fetching namespaces:', error);
      setError('Error fetching namespaces from the server.');
    }
  };

  const fetchData = async (namespace) => {
    try {
      const response = await axios.get(`/api/kubernetes/pods?namespace=${namespace}`);
      setRowData(response.data);
    } catch (error) {
      console.error('Error fetching data:', error);
      setError('Error fetching data from the server.');
    }
  };

  const openPodDetails = (podName) => {
    const url = `/#/kubernetes-pod-details?podName=${podName}&namespace=${selectedNamespace}`;
    window.open(url, '_blank');
  };

  return (
    <CContainer>
      {error && (
        <div className="alert alert-danger alert-dismissible fade show" role="alert">
          {error}
          <button type="button" className="btn-close" aria-label="Close" onClick={() => setError(null)}></button>
        </div>
      )}
      <CFormSelect value={selectedNamespace} onChange={(e) => setSelectedNamespace(e.target.value)}>
        {namespaces.map((namespace, index) => (
          <option key={index} value={namespace}>
            {namespace}
          </option>
        ))}
      </CFormSelect>
      <div style={{ height: 400, width: '100%' }}>
        <AgGridReact
          rowData={rowData}
          columnDefs={[
            { headerName: 'Name', field: 'name' },
            { headerName: 'Status', field: 'status' },
            {
              headerName: 'Actions',
              field: 'actions',
              cellRenderer: (params) => (
                <button onClick={() => openPodDetails(params.data.name)}>
                  View Pod Details
                </button>
              ),
            },
          ]}
          modules={[ClientSideRowModelModule]}
          defaultColDef={{ flex: 1, minWidth: 100, sortable: true, filter: true, resizable: true }}
          pagination={true}
          paginationPageSize={10}
          sideBar={{ toolPanels: ['columns', 'filters'] }}
        />
      </div>
      <CButton color="primary" style={{ marginTop: '10px' }}>Action Button</CButton>
    </CContainer>
  );
};

export default KubernetesPods;