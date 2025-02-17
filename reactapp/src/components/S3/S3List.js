import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton } from '@coreui/react';
import S3Modal from './S3Modal';
import DeleteConfirmationModal from './DeleteConfirmationModal';

const S3List = () => {
  const [rowData, setRowData] = useState([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [message, setMessage] = useState('');
  const [selectedRows, setSelectedRows] = useState([]);

  useEffect(() => {
    fetch('/api/s3/list')
      .then(response => response.json())
      .then(data => {
        const formattedData = data.map(bucket => ({
          bucketName: bucket.name,
          creationDate: bucket.creationDate
        }));
        setRowData(formattedData);
      });
  }, []);

  const columnDefs = [
    { headerName: 'Bucket Name', field: 'bucketName', filter: true, sortable: true, checkboxSelection: true },
    { headerName: 'Creation Date', field: 'creationDate', filter: true, sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  const handleCreate = async (bucketName) => {
    const response = await fetch('/api/s3/create', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ bucketName }),
    });
    const result = await response.text();
    setMessage(result);
    setShowModal(false);
    // Refresh the list after creating a new bucket
    fetch('/api/s3/list')
      .then(response => response.json())
      .then(data => {
        const formattedData = data.map(bucket => ({
          bucketName: bucket.name,
          creationDate: bucket.creationDate
        }));
        setRowData(formattedData);
      });
  };

  const handleDelete = async () => {
    const bucketNames = selectedRows.map(row => row.bucketName);
    const response = await fetch('/api/s3/deleteMultiple', {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(bucketNames),
    });
    const result = await response.text();
    setMessage(result);
    setShowDeleteModal(false);
    // Refresh the list after deleting buckets
    fetch('/api/s3/list')
      .then(response => response.json())
      .then(data => {
        const formattedData = data.map(bucket => ({
          bucketName: bucket.name,
          creationDate: bucket.creationDate
        }));
        setRowData(formattedData);
      });
  };

  return (
    <div>
      <h2>S3 Buckets</h2>
      <CButton color="primary" onClick={() => setShowModal(true)}>Create S3 Bucket</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Buckets</CButton>
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
          autoGroupColumnDef={{ headerName: 'Group', field: 'bucketName', cellRenderer: 'agGroupCellRenderer', cellRendererParams: { checkbox: true } }}
        />
      </div>
      <S3Modal
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

export default S3List;
