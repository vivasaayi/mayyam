import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import RegionDropdown from '../RegionDropdown';

const RdsClusters = () => {
  const [rowData, setRowData] = useState([]);
  const [region, setRegion] = useState('us-east-1');

  // Fetch list of DB clusters using the region.
  useEffect(() => {
    fetch(`/api/rds/clusters?region=${region}`)
      .then(response => response.json())
      .then(data => setRowData(data))
      .catch(err => console.error(err));
  }, [region]);

  const columnDefs = [
    { headerName: 'Cluster Identifier', field: 'clusterIdentifier', filter: true, sortable: true },
    { headerName: 'Status', field: 'status', filter: true, sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
  };

  return (
    (<div>
      <h2>RDS Clusters</h2>
      <RegionDropdown selectedRegion={region} onChange={e => setRegion(e.target.value)} />
      <div className="ag-theme-alpine" style={{ height: 600, width: '100%' }}>
        <AgGridReact
          columnDefs={columnDefs}
          rowData={rowData}
          rowSelection={{
            mode: 'multiRow'
          }}
          pagination={true}
          paginationPageSize={10}
          defaultColDef={defaultColDef}
        />
      </div>
    </div>)
  );
};

export default RdsClusters;