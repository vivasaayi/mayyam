import React, { useEffect, useState } from 'react';
import { CButton } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule } from 'ag-grid-community';
import axios from 'axios';

const PvcsTab = ({ namespace }) => {
  const [pvcsData, setPvcsData] = useState([]);
  const [error, setError] = useState(null);

  useEffect(() => {
    fetchPvcs();
  }, [namespace]);

  const fetchPvcs = async () => {
    try {
      const response = await axios.get(`/api/kubernetes/pvcs?namespace=${namespace}`);
      setPvcsData(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching PVCs:', error);
      setError('Error fetching PVCs');
    }
  };

  return (
    <div>
      <CButton color="primary" onClick={fetchPvcs}>Load</CButton>
      {error && <div className="alert alert-danger">{error}</div>}
      <div className="ag-theme-balham" style={{ height: 400, width: '100%', marginTop: '10px' }}>
        <AgGridReact
          rowData={pvcsData}
          columnDefs={[
            { headerName: 'Name', field: 'name' },
            { headerName: 'Status', field: 'status' },
            { headerName: 'Volume', field: 'volume' },
            { headerName: 'Capacity', field: 'capacity' },
            { headerName: 'Access Modes', field: 'accessModes' },
            { headerName: 'StorageClass', field: 'storageClass' },
            { headerName: 'VolumeAttributesClass', field: 'volumeAttributesClass' },
            { headerName: 'Age', field: 'age' },
          ]}
          defaultColDef={{ flex: 1, minWidth: 100 }}
          modules={[ClientSideRowModelModule]}
        />
      </div>
    </div>
  );
};

export default PvcsTab;
