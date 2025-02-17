import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CFormSelect, CAlert } from '@coreui/react';

const DynamoDbTablesWithoutPITR = () => {
  const [rowData, setRowData] = useState([]);
  const [message, setMessage] = useState('');
  const [messageType, setMessageType] = useState('success');
  const [region, setRegion] = useState('us-west-2');

  useEffect(() => {
    fetch(`/api/dynamodb/tablesWithoutPITR?region=${region}`)
      .then(response => response.json())
      .then(data => {
        setRowData(Object.entries(data).map(([tableName, tableStatus]) => ({ tableName, tableStatus })));
      })
      .catch(error => {
        setMessage(`Failed to fetch tables: ${error.message}`);
        setMessageType('danger');
      });
  }, [region]);

  const columnDefs = [
    { headerName: 'Table Name', field: 'tableName', filter: true, sortable: true },
    { headerName: 'Status', field: 'tableStatus', filter: true, sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  return (
    <div>
      <h2>DynamoDB Tables Without PITR</h2>
      <CFormSelect value={region} onChange={(e) => setRegion(e.target.value)}>
        <option value="us-west-2">US West (Oregon)</option>
        <option value="us-east-1">US East (N. Virginia)</option>
        <option value="eu-west-1">EU (Ireland)</option>
        {/* Add more regions as needed */}
      </CFormSelect>
      {message && <CAlert color={messageType}>{message}</CAlert>}
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

export default DynamoDbTablesWithoutPITR;