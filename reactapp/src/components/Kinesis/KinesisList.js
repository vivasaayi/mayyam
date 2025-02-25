import React, { useState, useEffect, useMemo } from 'react';
import { AgGridReact } from 'ag-grid-react';
import { CButton, CAlert } from '@coreui/react';
import KinesisModal from './KinesisModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';
import RegionDropdown from '../RegionDropdown';
import { ModuleRegistry, AllCommunityModule } from "ag-grid-community";
ModuleRegistry.registerModules([AllCommunityModule]);

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

  const columnDefs = [
    { headerName: 'Stream Name', field: 'streamName', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Stream ARN', field: 'streamARN', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Stream Status', field: 'streamStatus', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Shards', field: 'shards.length', filter: 'agNumberColumnFilter', sortable: true }
  ];

  const defaultColDef = useMemo(() => ({
    flex: 1,
    minWidth: 100,
  }), []);

  const rowSelection = useMemo(() => {
    return {
      mode: "multiRow",
    };
  }, []);

  const handleCreate = async (streamName, shardCount) => {
    const response = await fetch(`/api/kinesis/create?streamName=${streamName}&shardCount=${shardCount}&region=${region}`, {
      method: 'POST'
    });
    const result = await response.text();
    setMessage(result);
    setMessageType('success');
    setShowModal(false);
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
