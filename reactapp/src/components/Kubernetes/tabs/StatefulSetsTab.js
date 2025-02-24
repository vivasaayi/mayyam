import React, { useEffect, useState } from 'react';
import { CButton } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import axios from 'axios';
import {
  ClientSideRowModelModule,
  DateFilterModule,
  ModuleRegistry,
  NumberFilterModule,
  TextFilterModule,
  ValidationModule,
} from 'ag-grid-community';

ModuleRegistry.registerModules([
  ClientSideRowModelModule,
  TextFilterModule,
  NumberFilterModule,
  DateFilterModule,
  ValidationModule,
]);

const StatefulSetsTab = ({ namespace }) => {
  const [statefulSetsData, setStatefulSetsData] = useState([]);
  const [error, setError] = useState(null);

  useEffect(() => {
    fetchStatefulSets();
  }, [namespace]);

  const fetchStatefulSets = async () => {
    try {
      const response = await axios.get(`/api/kubernetes/statefulsets?namespace=${namespace}`);
      setStatefulSetsData(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching stateful sets:', error);
      setError('Error fetching stateful sets');
    }
  };

  return (
    <div>
      <CButton color="primary" onClick={fetchStatefulSets}>Load</CButton>
      {error && <div className="alert alert-danger">{error}</div>}
      <div className="ag-theme-balham" style={{ height: 400, width: '100%', marginTop: '10px' }}>
        <AgGridReact
          rowData={statefulSetsData}
          columnDefs={[
            { headerName: 'Name', field: 'name' },
            { headerName: 'Expected Replicas', field: 'expectedReplicas' },
            { headerName: 'Pods Running', field: 'podsRunning' },
            { headerName: 'Pods Pending', field: 'podsPending' },
            { headerName: 'Pods Not Started', field: 'podsNotStarted' },
            { headerName: 'Actions', field: 'actions', cellRendererFramework: (params) => <CButton color="primary">View Pod Details</CButton> },
          ]}
          defaultColDef={{ flex: 1, minWidth: 100, sortable: true, filter: true, resizable: true }}
          pagination={true}
          paginationPageSize={10}
          sideBar={{ toolPanels: ['columns', 'filters'] }}
          modules={[ClientSideRowModelModule]}
        />
      </div>
    </div>
  );
};

export default StatefulSetsTab;
