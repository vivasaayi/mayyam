import React, { useState, useEffect } from 'react';
import { CButton, CAlert } from '@coreui/react';
import KinesisModal from './KinesisModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';
import RegionDropdown from '../RegionDropdown';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule } from 'ag-grid-community';

const KinesisList = () => {
  const [rowData, setRowData] = useState([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [message, setMessage] = useState('');
  const [messageType, setMessageType] = useState('success');
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
      })
      .catch(error => {
        setMessage(`Failed to fetch streams: ${error.message}`);
        setMessageType('danger');
      });
  }, [region]);

  const columns = [
    { headerName: 'Stream Name', field: 'streamName', width: 200 },
    { headerName: 'Stream ARN', field: 'streamARN', width: 300 },
    { headerName: 'Stream Status', field: 'streamStatus', width: 150 },
    { headerName: 'Shards', field: 'shards.length', width: 100 }
  ];

  const handleCreate = async (streamName, shardCount) => {
    const response = await fetch(`/api/kinesis/create?streamName=${streamName}&shardCount=${shardCount}&region=${region}`, {
      method: 'POST'
    });
    const result = await response.text();
    setMessage(result);
    setMessageType('success');
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
    setMessageType('success');
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
      <RegionDropdown selectedRegion={region} onChange={(e) => setRegion(e.target.value)} />
      <CButton color="primary" onClick={() => setShowModal(true)}>Create Kinesis Stream</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Streams</CButton>
      {message && <CAlert color={messageType}>{message}</CAlert>}
      <div className="ag-theme-balham" style={{ height: '600px', width: '100%' }}>
        <AgGridReact
          rowData={rowData}
          columnDefs={columns}
          rowSelection="multiple"
          onSelectionChanged={(event) => setSelectedRows(event.api.getSelectedRows())}
          modules={[ClientSideRowModelModule]}
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
