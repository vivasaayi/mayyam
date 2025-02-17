import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton } from '@coreui/react';
import DynamoDbModal from './DynamoDbModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';

const DynamoDbList = () => {
  const [rowData, setRowData] = useState([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [message, setMessage] = useState('');
  const [selectedRows, setSelectedRows] = useState([]);

  useEffect(() => {
    fetch('/api/dynamodb/list')
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  }, []);

  const columnDefs = [
    { headerName: 'Table Name', field: 'tableName', filter: true, sortable: true, checkboxSelection: true },
    { headerName: 'Table Status', field: 'tableStatus', filter: true, sortable: true },
    { headerName: 'Item Count', field: 'itemCount', filter: true, sortable: true },
    { headerName: 'Table Size (Bytes)', field: 'tableSizeBytes', filter: true, sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  const handleCreate = async (tableName, attributeDefinitions, keySchema, provisionedThroughput) => {
    const response = await fetch('/api/dynamodb/create', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ tableName, attributeDefinitions, keySchema, provisionedThroughput }),
    });
    const result = await response.text();
    setMessage(result);
    setShowModal(false);
    // Refresh the list after creating a new table
    fetch('/api/dynamodb/list')
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  };

  const handleDelete = async () => {
    const tableNames = selectedRows.map(row => row.tableName);
    const response = await fetch('/api/dynamodb/deleteMultiple', {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(tableNames),
    });
    const result = await response.text();
    setMessage(result);
    setShowDeleteModal(false);
    // Refresh the list after deleting tables
    fetch('/api/dynamodb/list')
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  };

  const handleReplicationStatus = () => {
    window.open('/#/dynamodb/replication', '_blank');
  };

  return (
    <div>
      <h2>DynamoDB Tables</h2>
      <CButton color="primary" onClick={() => setShowModal(true)}>Create DynamoDB Table</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Tables</CButton>
      <CButton color="info" onClick={handleReplicationStatus}>See Replication Status</CButton>
      {message && <p>{message}</p>}
      <div className="ag-theme-alpine" style={{ height: 600, width: '100%' }}>
        <AgGridReact
          columnDefs={columnDefs}
          rowData={rowData}
          rowSelection="multiple"
          onSelectionChanged={(event) => setSelectedRows(event.api.getSelectedRows())}
          pagination={true}
          paginationPageSize={10}
          domLayout='autoHeight'
          defaultColDef={defaultColDef}
          groupSelectsChildren={true}
          autoGroupColumnDef={{ headerName: 'Group', field: 'tableName', cellRenderer: 'agGroupCellRenderer', cellRendererParams: { checkbox: true } }}
        />
      </div>
      <DynamoDbModal
        show={showModal}
        handleClose={() => setShowModal(false)}
        handleCreate={handleCreate}
      />
      <DeleteConfirmationModal
        show={showDeleteModal}
        handleClose={() => setShowDeleteModal(false)}
        handleConfirm={handleDelete}
        selectedStreams={selectedRows}
      />
    </div>
  );
};

export default DynamoDbList;
