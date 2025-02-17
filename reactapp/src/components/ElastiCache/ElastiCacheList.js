import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton, CFormSelect } from '@coreui/react';
// import ElastiCacheModal from './ElastiCacheModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';

const ElastiCacheList = () => {
  const [rowData, setRowData] = useState([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [message, setMessage] = useState('');
  const [selectedRows, setSelectedRows] = useState([]);
  const [region, setRegion] = useState('us-west-2');

  useEffect(() => {
    fetch(`/api/elasticache/list?region=${region}`)
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  }, [region]);

  const columnDefs = [
    { headerName: 'Cluster ID', field: 'cacheClusterId', filter: true, sortable: true, checkboxSelection: true },
    { headerName: 'Engine', field: 'engine', filter: true, sortable: true },
    { headerName: 'Node Type', field: 'cacheNodeType', filter: true, sortable: true },
    { headerName: 'Status', field: 'cacheClusterStatus', filter: true, sortable: true }
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  const handleCreate = async (clusterId, properties) => {
    const response = await fetch(`/api/elasticache/create?clusterId=${clusterId}&region=${region}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(properties),
    });
    const result = await response.text();
    setMessage(result);
    setShowModal(false);
    // Refresh the list after creating a new cluster
    fetch(`/api/elasticache/list?region=${region}`)
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  };

  const handleDelete = async () => {
    const clusterIdsAndRegions = selectedRows.reduce((acc, row) => {
      acc[row.cacheClusterId] = region;
      return acc;
    }, {});
    const response = await fetch('/api/elasticache/deleteMultiple', {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(clusterIdsAndRegions),
    });
    const result = await response.text();
    setMessage(result);
    setShowDeleteModal(false);
    // Refresh the list after deleting clusters
    fetch(`/api/elasticache/list?region=${region}`)
      .then(response => response.json())
      .then(data => {
        setRowData(data);
      });
  };

  return (
    <div>
      <h2>ElastiCache Clusters</h2>
      <CFormSelect value={region} onChange={(e) => setRegion(e.target.value)}>
        <option value="us-west-2">US West (Oregon)</option>
        <option value="us-east-1">US East (N. Virginia)</option>
        <option value="eu-west-1">EU (Ireland)</option>
        {/* Add more regions as needed */}
      </CFormSelect>
      <CButton color="primary" onClick={() => setShowModal(true)}>Create ElastiCache Cluster</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Clusters</CButton>
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
          autoGroupColumnDef={{ headerName: 'Group', field: 'cacheClusterId', cellRenderer: 'agGroupCellRenderer', cellRendererParams: { checkbox: true } }}
        />
      </div>
      <ElastiCacheModal
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

export default ElastiCacheList;
