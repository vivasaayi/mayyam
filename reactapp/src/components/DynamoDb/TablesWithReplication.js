import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import { CButton } from '@coreui/react';
import RegionDropdown from '../RegionDropdown';

// Import AG Grid modules
import { ClientSideRowModelModule, DateFilterModule, ModuleRegistry, NumberFilterModule, TextFilterModule, ValidationModule } from "ag-grid-community";

// Register AG Grid modules
ModuleRegistry.registerModules([ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]);

const TablesWithReplication = () => {
  const [rowData, setRowData] = useState([]);
  const [region, setRegion] = useState('us-west-2');

  useEffect(() => {
    fetch(`/api/dynamodb/tablesWithReplication?region=${region}`)
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  }, [region]);

  const columnDefs = [
    { headerName: 'Table Name', field: 'tableName', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Status', field: 'status', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Replicas', field: 'replicas', filter: 'agNumberColumnFilter', sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  const exportToCsv = () => {
    const csvData = rowData.map(row => ({
      'Table Name': row.tableName,
      'Status': row.status,
      'Replicas': row.replicas
    }));
    const csvContent = [
      ['Table Name', 'Status', 'Replicas'],
      ...csvData.map(row => [row['Table Name'], row['Status'], row['Replicas']])
    ].map(e => e.join(",")).join("\n");

    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    const link = document.createElement("a");
    const url = URL.createObjectURL(blob);
    link.setAttribute("href", url);
    link.setAttribute("download", "tables_with_replication.csv");
    link.style.visibility = 'hidden';
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  return (
    <div>
      <h2>Tables With Global Replication</h2>
      <RegionDropdown selectedRegion={region} onChange={(e) => setRegion(e.target.value)} />
      <CButton color="primary" onClick={exportToCsv}>Export to CSV</CButton>
      <div className="ag-theme-alpine" style={{ height: 600, width: '100%' }}>
        <AgGridReact
          columnDefs={columnDefs}
          rowData={rowData}
          pagination={true}
          paginationPageSize={10}
          domLayout='autoHeight'
          defaultColDef={defaultColDef}
          modules={[ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]}
        />
      </div>
    </div>
  );
};

export default TablesWithReplication;
