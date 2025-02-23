import React, { useEffect, useState } from 'react';
import { CButton } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule } from 'ag-grid-community';
import axios from 'axios';

const StorageClassesTab = ({ namespace }) => {
  const [storageClassesData, setStorageClassesData] = useState([]);

  useEffect(() => {
    fetchStorageClasses();
  }, [namespace]);

  const fetchStorageClasses = async () => {
    try {
      const response = await axios.get(`/api/kubernetes/storageclasses?namespace=${namespace}`);
      setStorageClassesData(response.data);
    } catch (error) {
      console.error('Error fetching storage classes:', error);
    }
  };

  return (
    <div>
      <CButton color="primary" onClick={fetchStorageClasses}>Load</CButton>
      <div className="ag-theme-balham" style={{ height: 400, width: '100%', marginTop: '10px' }}>
        <AgGridReact
          rowData={storageClassesData}
          columnDefs={[
            { headerName: 'Name', field: 'name' },
            { headerName: 'Expected Replicas', field: 'expectedReplicas' },
            { headerName: 'Pods Running', field: 'podsRunning' },
            { headerName: 'Pods Pending', field: 'podsPending' },
            { headerName: 'Pods Not Started', field: 'podsNotStarted' },
            { headerName: 'Actions', field: 'actions', cellRendererFramework: (params) => <CButton color="primary">View Pod Details</CButton> },
          ]}
          defaultColDef={{ flex: 1, minWidth: 100 }}
          modules={[ClientSideRowModelModule]}
        />
      </div>
    </div>
  );
};

export default StorageClassesTab;
