import React, { useEffect, useState } from 'react';
import { CContainer, CButton } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule } from 'ag-grid-community';
import axios from 'axios';

const KubernetesPods = () => {
  const [rowData, setRowData] = useState([]);
  const [error, setError] = useState(null);

  useEffect(() => {
    fetchData();
  }, []);

  const fetchData = async () => {
    try {
      const response = await axios.get('/api/kubernetes/pods?namespace=default');
      setRowData(response.data);
    } catch (error) {
      console.error('Error fetching data:', error);
      setError('Error fetching data from the server.');
    }
  };

  return (
    <CContainer>
      {error && (
        <div className="alert alert-danger alert-dismissible fade show" role="alert">
          {error}
          <button type="button" className="btn-close" aria-label="Close" onClick={() => setError(null)}></button>
        </div>
      )}
      <div style={{ height: 400, width: '100%' }}>
        <AgGridReact
          rowData={rowData}
          columnDefs={[
            { headerName: 'Name', field: 'name' },
            { headerName: 'Status', field: 'status' },
            { headerName: 'Actions', field: 'actions', cellRendererFramework: (params) => <button>View Pod Details</button> },
          ]}
          modules={[ClientSideRowModelModule]}
        />
      </div>
      <CButton color="primary" style={{ marginTop: '10px' }}>Action Button</CButton>
    </CContainer>
  );
};

export default KubernetesPods;