import React, { useEffect, useState } from 'react';
import { CContainer, CButton } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import axios from 'axios';

const KubernetesPods = () => {
  const [rowData, setRowData] = useState([]);

  useEffect(() => {
    fetchData();
  }, []);

  const fetchData = async () => {
    try {
      const response = await axios.get('/api/kubernetes/pods?namespace=default');
      setRowData(response.data);
    } catch (error) {
      console.error('Error fetching data:', error);
    }
  };

  return (
    <CContainer>
      <div className="ag-theme-alpine" style={{ height: 400, width: '100%' }}>
        <AgGridReact
          rowData={rowData}
          columnDefs={[
            { headerName: 'Name', field: 'name' },
            { headerName: 'Status', field: 'status' },
            { headerName: 'Actions', field: 'actions', cellRendererFramework: (params) => <button>View Pod Details</button> },
          ]}
        />
      </div>
      <CButton color="primary" style={{ marginTop: '10px' }}>Action Button</CButton>
    </CContainer>
  );
};

export default KubernetesPods;