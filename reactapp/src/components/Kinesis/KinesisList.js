import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton, CFormSelect } from '@coreui/react';
import KinesisModal from './KinesisModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';

const KinesisList = () => {
  const [rowData, setRowData] = useState([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [message, setMessage] = useState('');
  const [selectedRows, setSelectedRows] = useState([]);
  const [region, setRegion] = useState('us-west-2');

  useEffect(() => {
    fetch(`/api/kinesis/list?region=${region}`)
      .then(response => response.json())
      .then(data => {
        const formattedData = Object.keys(data).map(key => ({
          streamName: key,
          ...data[key]
        }));
        setRowData(formattedData);
      });
  }, [region]);

  const columnDefs = [
    { headerName: 'Stream Name', field: 'streamName', filter: true, sortable: true, checkboxSelection: true },
    { headerName: 'Stream ARN', field: 'streamARN', filter: true, sortable: true },
    { headerName: 'Stream Status', field: 'streamStatus', filter: true, sortable: true },
    { headerName: 'Shards', field: 'shards.length', filter: true, sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  const handleCreate = async (streamName, shardCount) => {
    const response = await fetch(`/api/kinesis/create?streamName=${streamName}&shardCount=${shardCount}&region=${region}`, {
      method: 'POST'
    });
    const result = await response.text();
    setMessage(result);
    setShowModal(false);
    // Refresh the list after creating a new stream
    fetch(`/api/kinesis/list?region=${region}`)
      .then(response => response.json())
      .then(data => {
        const formattedData = Object.keys(data).map(key => ({
          streamName: key,
          ...data[key]
        }));
        setRowData(formattedData);
      });
  };

  const handleDelete = async () => {
    const streamNamesAndRegions = selectedRows.reduce((acc, row) => {
      acc[row.streamName] = region;
      return acc;
    }, {});
    const response = await fetch('/api/kinesis/deleteMultiple', {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(streamNamesAndRegions),
    });
    const result = await response.text();
    setMessage(result);
    setShowDeleteModal(false);
    // Refresh the list after deleting streams
    fetch(`/api/kinesis/list?region=${region}`)
      .then(response => response.json())
      .then(data => {
        const formattedData = Object.keys(data).map(key => ({
          streamName: key,
          ...data[key]
        }));
        setRowData(formattedData);
      });
  };

  return (
    <div>
      <h2>Kinesis Streams</h2>
      <CFormSelect value={region} onChange={(e) => setRegion(e.target.value)}>
        <option value="us-west-2">US West (Oregon)</option>
        <option value="us-east-1">US East (N. Virginia)</option>
        <option value="eu-west-1">EU (Ireland)</option>
        {/* Add more regions as needed */}
      </CFormSelect>
      <CButton color="primary" onClick={() => setShowModal(true)}>Create Kinesis Stream</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Streams</CButton>
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
          autoGroupColumnDef={{ headerName: 'Group', field: 'streamName', cellRenderer: 'agGroupCellRenderer', cellRendererParams: { checkbox: true } }}
        />
      </div>
      <KinesisModal
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

export default KinesisList;
