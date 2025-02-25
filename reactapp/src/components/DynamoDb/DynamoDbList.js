import React, { useState, useEffect, useMemo } from 'react';
import { AgGridReact } from 'ag-grid-react';
import { CButton, CAlert } from '@coreui/react';
import DynamoDbModal from './DynamoDbModal';
import DeleteConfirmationModal from './DeleteConfirmationModal';
import RegionDropdown from '../RegionDropdown';

// Import AG Grid modules
import { ModuleRegistry, AllCommunityModule } from "ag-grid-community";

// Register AG Grid modules
ModuleRegistry.registerModules([AllCommunityModule]);

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
    { headerName: 'Table Name', field: 'tableName', filter: 'agTextColumnFilter', sortable: true, },
    { headerName: 'Status', field: 'tableStatus', filter: 'agTextColumnFilter', sortable: true },
    { headerName: 'Item Count', field: 'itemCount', filter: 'agNumberColumnFilter', sortable: true },
    { headerName: 'Size (Bytes)', field: 'tableSizeBytes', filter: 'agNumberColumnFilter', sortable: true }
  ];

  const defaultColDef = useMemo(() => {
    return {
      flex: 1,
      minWidth: 100,
    };
  }, []);

  const rowSelection = useMemo(() => {
    return {
      mode: "multiRow",
    };
  }, []);

  const handleCreate = async (properties) => {
    try {
      const response = await fetch(`/api/dynamodb/create?tableName=${properties.tableName}&region=${region}`, {
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
      const tableNames = selectedRows.map(row => row.tableName);
      const response = await fetch(`/api/dynamodb/deleteMultiple?region=${region}`, {
        method: 'DELETE',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(tableNames),
      });
      const result = await response.text();
      setMessage(result);
      setMessageType('success');
      setShowDeleteModal(false);
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
    window.open(`/#/dynamodb/replication?region=${region}`, '_blank');
  };

  const handleViewTablesWithoutPITR = () => {
    window.open(`/#/dynamodb/tablesWithoutPITR?region=${region}`, '_blank');
  };

  return (
    <div>
      <h2>DynamoDB Tables</h2>
      <RegionDropdown selectedRegion={region} onChange={(e) => setRegion(e.target.value)} />
      <CButton color="primary" onClick={() => setShowModal(true)}>Create DynamoDB Table</CButton>
      <CButton color="danger" onClick={() => setShowDeleteModal(true)} disabled={selectedRows.length === 0}>Delete Selected Tables</CButton>
      <CButton color="info" onClick={handleReplicationStatus}>See Replication Status</CButton>
      <CButton color="warning" onClick={handleViewTablesWithoutPITR}>View Tables without PITR</CButton>
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
