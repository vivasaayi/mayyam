import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton } from '@coreui/react';
import SqsModal from './SqsModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';

const SqsList = () => {
  const [rowData, setRowData] = useState([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [message, setMessage] = useState('');
  const [selectedRows, setSelectedRows] = useState([]);

  useEffect(() => {
    fetch('/api/sqs/list')
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  }, []);

  const columnDefs = [
    { headerName: 'Queue Name', field: 'queueName', filter: true, sortable: true, checkboxSelection: true },
    { headerName: 'Queue URL', field: 'queueUrl', filter: true, sortable: true },
    { headerName: 'Attributes', field: 'attributes', filter: true, sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  const handleCreate = async (queueName) => {
    const response = await fetch(`/api/sqs/create?queueName=${queueName}`, {
      method: 'POST'
    });
    const result = await response.text();
    setMessage(result);
    setShowModal(false);
    // Refresh the list after creating a new queue
    fetch('/api/sqs/list')
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  };

  const handleDelete = async () => {
    const queueUrls = selectedRows.map(row => row.queueUrl);
    const response = await fetch('/api/sqs/deleteMultiple', {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(queueUrls),
    });
    const result = await response.text();
    setMessage(result);
    setShowDeleteModal(false);
    // Refresh the list after deleting queues
    fetch('/api/sqs/list')
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  };

  return (
    <div>
      <h2>SQS Queues</h2>
      <CButton color="primary" onClick={() => setShowModal(true)}>Create SQS Queue</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Queues</CButton>
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
          autoGroupColumnDef={{ headerName: 'Group', field: 'queueName', cellRenderer: 'agGroupCellRenderer', cellRendererParams: { checkbox: true } }}
        />
      </div>
      <SqsModal
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

export default SqsList;
