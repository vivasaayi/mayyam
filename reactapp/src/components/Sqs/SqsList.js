import React, { useState, useEffect, useMemo } from 'react';
import { AgGridReact } from 'ag-grid-react';
import { CButton, CAlert } from '@coreui/react';
import SqsModal from './SqsModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';
import RegionDropdown from '../RegionDropdown';

// Import AG Grid modules
import { 
  ModuleRegistry, AllCommunityModule
 } from "ag-grid-community";

// Register AG Grid modules
ModuleRegistry.registerModules([AllCommunityModule]);

const SqsList = () => {
  const [rowData, setRowData] = useState([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [message, setMessage] = useState('');
  const [messageType, setMessageType] = useState('success');
  const [selectedRows, setSelectedRows] = useState([]);
  const [region, setRegion] = useState('us-west-2');

  useEffect(() => {
    fetch(`/api/sqs/listWithStatus?region=${region}`)
      .then(response => response.json())
      .then(data => {
        const formattedData = Object.entries(data).map(([queueName, details]) => ({
          queueName,
          queueUrl: details.queueUrl,
          status: details.status
        }));
        setRowData(formattedData);
      })
      .catch(error => {
        setMessage(`Failed to fetch queues: ${error.message}`);
        setMessageType('danger');
      });
  }, [region]);

  const columnDefs = [
    { headerName: 'Queue Name', field: 'queueName', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Queue URL', field: 'queueUrl', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Status', field: 'status', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Attributes', field: 'attributes', filter: 'agTextColumnFilter', sortable: true }
  ];

  const defaultColDef = useMemo(() => ({
    flex: 1,
    minWidth: 100,
  }), []);

  const rowSelection = useMemo(() => ({
    mode: 'multiRow',
  }), []);

  const handleCreate = async (queueName) => {
    const response = await fetch(`/api/sqs/create?queueName=${queueName}&region=${region}`, {
      method: 'POST'
    });
    const result = await response.text();
    setMessage(result);
    setMessageType('success');
    setShowModal(false);
    // Refresh the list after creating a new queue
    fetch(`/api/sqs/listWithStatus?region=${region}`)
      .then(response => response.json())
      .then(data => {
        const formattedData = Object.entries(data).map(([queueName, details]) => ({
          queueName,
          queueUrl: details.queueUrl,
          status: details.status
        }));
        setRowData(formattedData);
      });
  };

  const handleDelete = async () => {
    const queueUrls = selectedRows.map(row => row.queueUrl);
    const response = await fetch(`/api/sqs/deleteMultiple?region=${region}`, {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(queueUrls),
    });
    const result = await response.text();
    setMessage(result);
    setMessageType('success');
    setShowDeleteModal(false);
    fetch(`/api/sqs/listWithStatus?region=${region}`)
      .then(response => response.json())
      .then(data => {
        const formattedData = Object.entries(data).map(([queueName, details]) => ({
          queueName,
          queueUrl: details.queueUrl,
          status: details.status
        }));
        setRowData(formattedData);
      });
  };

  return (
    <div>
      <h2>SQS Queues</h2>
      <RegionDropdown selectedRegion={region} onChange={(e) => setRegion(e.target.value)} />
      <CButton color="primary" onClick={() => setShowModal(true)}>Create SQS Queue</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Queues</CButton>
      {message && <CAlert color={messageType}>{message}</CAlert>}
      <div className="ag-theme-alpine" style={{ height: 600, width: '100%' }}>
        <AgGridReact
          columnDefs={columnDefs}
          rowData={rowData}
          onSelectionChanged={(event) => setSelectedRows(event.api.getSelectedRows())}
          pagination={true}
          paginationPageSize={10}
          domLayout='autoHeight'
          defaultColDef={defaultColDef}
          modules={[AllCommunityModule]}
          rowSelection={rowSelection}
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
        message={`Are you sure you want to delete the following queues? ${selectedRows.map(row => row.queueUrl).join(', ')}`}
      />
    </div>
  );
};

export default SqsList;
