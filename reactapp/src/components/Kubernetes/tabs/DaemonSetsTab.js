import React, { useEffect, useState } from 'react';
import { CButton } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule } from 'ag-grid-community';
import axios from 'axios';

const DaemonSetsTab = ({ namespace }) => {
  const [daemonSetsData, setDaemonSetsData] = useState([]);
  const [error, setError] = useState(null);

  useEffect(() => {
    fetchDaemonSets();
  }, [namespace]);

  const fetchDaemonSets = async () => {
    try {
      const response = await axios.get(`/api/kubernetes/daemonsets?namespace=${namespace}`);
      setDaemonSetsData(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching daemon sets:', error);
      setError('Error fetching daemon sets');
    }
  };

  return (
    <div>
      <CButton color="primary" onClick={fetchDaemonSets}>Load</CButton>
      {error && <div className="alert alert-danger">{error}</div>}
      <div className="ag-theme-balham" style={{ height: 400, width: '100%', marginTop: '10px' }}>
        <AgGridReact
          rowData={daemonSetsData}
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

export default DaemonSetsTab;
