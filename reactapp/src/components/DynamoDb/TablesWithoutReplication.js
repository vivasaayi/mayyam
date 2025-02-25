import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton } from '@coreui/react';
import RegionDropdown from '../RegionDropdown';

const TablesWithoutReplication = () => {
  const [rowData, setRowData] = useState([]);
  const [region, setRegion] = useState('us-west-2');

  useEffect(() => {
    fetch(`/api/dynamodb/tablesWithoutReplication?region=${region}`)
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  }, [region]);

  const columnDefs = [
    { headerName: 'Table Name', field: 'tableName', filter: true, sortable: true },
    { headerName: 'Status', field: 'status', filter: true, sortable: true }
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
      'Status': row.status
    }));
    const csvContent = [
      ['Table Name', 'Status'],
      ...csvData.map(row => [row['Table Name'], row['Status']])
    ].map(e => e.join(",")).join("\n");

    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    const link = document.createElement("a");
    const url = URL.createObjectURL(blob);
    link.setAttribute("href", url);
    link.setAttribute("download", "tables_without_replication.csv");
    link.style.visibility = 'hidden';
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  return (
    <div>
      <h2>Tables Without Global Replication</h2>
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
        />
      </div>
    </div>
  );
};

export default TablesWithoutReplication;
