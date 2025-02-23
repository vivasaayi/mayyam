import React, { useEffect, useState } from 'react';
import { CContainer, CButton, CFormSelect } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule } from 'ag-grid-community';

import axios from 'axios';

const KubernetesDashboard = () => {
  const [deploymentsData, setDeploymentsData] = useState([]);
  const [namespaces, setNamespaces] = useState([]);
  const [selectedNamespace, setSelectedNamespace] = useState('');

  useEffect(() => {
    fetchNamespaces();
  }, []);

  useEffect(() => {
    fetchDeployments();
  }, [selectedNamespace]);

  useEffect(() => {
    console.log('Namespaces:', namespaces);
  }, [namespaces]);

  useEffect(() => {
    console.log('Deployments Data:', deploymentsData);
  }, [deploymentsData]);

  const fetchNamespaces = async () => {
    try {
      const response = await axios.get('/api/kubernetes/namespaces');
      setNamespaces(response.data);
      setSelectedNamespace(response.data[0]);
    } catch (error) {
      console.error('Error fetching namespaces:', error);
    }
  };

  const fetchDeployments = async () => {
    try {
      const response = await axios.get(`/api/kubernetes/deployments?namespace=${selectedNamespace}`);
      setDeploymentsData(response.data);
    } catch (error) {
      console.error('Error fetching deployments:', error);
    }
  };

  const renderDeploymentsGrid = () => {
    return (
      <>
        <div className="ag-theme-balham" style={{ height: 400, width: '100%' }}>
          <AgGridReact
            rowData={deploymentsData}
            columnDefs={[
              { headerName: 'Name', field: 'name' },
              { headerName: 'Expected Replicas', field: 'expectedReplicas' },
              { headerName: 'Pods Running', field: 'podsRunning' },
              { headerName: 'Pods Pending', field: 'podsPending' },
              { headerName: 'Pods Not Started', field: 'podsNotStarted' },
              { headerName: 'Actions', field: 'actions' },
            ]}
            defaultColDef={{ flex: 1, minWidth: 100 }}
            modules={[ClientSideRowModelModule]}
          />
        </div>
        <CButton color="primary" style={{ marginTop: '10px' }}>Action Button</CButton>
      </>
    );
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
      {renderDeploymentsGrid()}
      {/* Add more grid rendering as needed */}
    </CContainer>
  );
};

export default KubernetesDashboard;