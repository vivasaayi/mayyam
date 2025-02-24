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

const StorageClassesTab = ({ namespace }) => {
  const [storageClassesData, setStorageClassesData] = useState([]);
  const [error, setError] = useState(null);

  useEffect(() => {
    fetchStorageClasses();
  }, [namespace]);

  const fetchStorageClasses = async () => {
    try {
      const response = await axios.get(`/api/kubernetes/storageclasses`);
      setStorageClassesData(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching storage classes:', error);
      setError('Error fetching storage classes');
    }
  };

  return (
    <div>
      <CButton color="primary" onClick={fetchStorageClasses}>Load</CButton>
      {error && <div className="alert alert-danger">{error}</div>}
      <div className="ag-theme-balham" style={{ height: 400, width: '100%', marginTop: '10px' }}>
        <AgGridReact
          rowData={storageClassesData}
          columnDefs={[
            { headerName: 'Name', field: 'name' },
            { headerName: 'Provisioner', field: 'provisioner' },
            { headerName: 'Reclaim Policy', field: 'reclaimPolicy' },
            { headerName: 'Volume Binding Mode', field: 'volumeBindingMode' },
            { headerName: 'Allow Volume Expansion', field: 'allowVolumeExpansion' },
            { headerName: 'Age', field: 'age' },
          ]}
          defaultColDef={{ flex: 1, minWidth: 100, sortable: true, filter: true, resizable: true }}
          pagination={true}
          paginationPageSize={10}
          sideBar={{ toolPanels: ['columns', 'filters'] }}
          modules={[ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]}
        />
      </div>
    </div>
  );
};

export default StorageClassesTab;
