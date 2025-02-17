import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton, CFormSelect, CAlert } from '@coreui/react';
import DynamoDbModal from './DynamoDbModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';

const DynamoDbList = () => {
  const [rowData, setRowData] = useState([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [message, setMessage] = useState('');
  const [messageType, setMessageType] = useState('success');
  const [selectedRows, setSelectedRows] = useState([]);
  const [region, setRegion] = useState('us-west-2');

  useEffect(() => {
    fetch(`/api/dynamodb/list?region=${region}`)
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      })
      .catch(error => {
        setMessage(`Failed to fetch tables: ${error.message}`);
        setMessageType('danger');
      });
  }, [region]);

  const columnDefs = [
    { headerName: 'Table Name', field: 'tableName', filter: true, sortable: true, checkboxSelection: true },
    { headerName: 'Status', field: 'tableStatus', filter: true, sortable: true },
    { headerName: 'Item Count', field: 'itemCount', filter: true, sortable: true },
    { headerName: 'Size (Bytes)', field: 'tableSizeBytes', filter: true, sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  const handleCreate = async (tableName, properties) => {
    try {
      const response = await fetch(`/api/dynamodb/create?tableName=${tableName}&region=${region}`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(properties),
      });
      const result = await response.text();
      setMessage(result);
      setMessageType('success');
      setShowModal(false);
      // Refresh the list after creating a new table
      fetch(`/api/dynamodb/list?region=${region}`)
        .then(response => response.json())
        .then(data => {
          setRowData(data);
        });
    } catch (error) {
      setMessage(`Failed to create table: ${error.message}`);
      setMessageType('danger');
    }
  };

  const handleDelete = async () => {
    try {
      const tableNamesAndRegions = selectedRows.reduce((acc, row) => {
        acc[row.tableName] = region;
        return acc;
      }, {});
      const response = await fetch('/api/dynamodb/deleteMultiple', {
        method: 'DELETE',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(tableNamesAndRegions),
      });
      const result = await response.text();
      setMessage(result);
      setMessageType('success');
      setShowDeleteModal(false);
      // Refresh the list after deleting tables
      fetch(`/api/dynamodb/list?region=${region}`)
        .then(response => response.json())
        .then(data => {
          setRowData(data);
        });
    } catch (error) {
      setMessage(`Failed to delete tables: ${error.message}`);
      setMessageType('danger');
    }
  };

  const handleReplicationStatus = () => {
    window.open('/#/dynamodb/replication', '_blank');
  };

  const handleViewTablesWithoutPITR = () => {
    window.open(`/dynamodb/tablesWithoutPITR?region=${region}`, '_blank');
  };

  return (
    <div>
      <h2>DynamoDB Tables</h2>
      <CFormSelect value={region} onChange={(e) => setRegion(e.target.value)}>
        <option value="us-west-2">US West (Oregon)</option>
        <option value="us-east-1">US East (N. Virginia)</option>
        <option value="eu-west-1">EU (Ireland)</option>
        {/* Add more regions as needed */}
      </CFormSelect>
      <CButton color="primary" onClick={() => setShowModal(true)}>Create DynamoDB Table</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Tables</CButton>
      <CButton color="info" onClick={handleReplicationStatus}>See Replication Status</CButton>
      <CButton color="warning" onClick={handleViewTablesWithoutPITR}>View Tables without PITR</CButton>
      {message && <CAlert color={messageType}>{message}</CAlert>}
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
