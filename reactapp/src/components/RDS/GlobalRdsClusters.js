import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton, CAlert, CSpinner } from '@coreui/react';
import RegionDropdown from '../RegionDropdown';

const GlobalRdsClusters = () => {
  const [rowData, setRowData] = useState([]);
  const [region, setRegion] = useState('us-west-2');
  const [message, setMessage] = useState('');
  const [messageType, setMessageType] = useState('success');
  const [loading, setLoading] = useState(false);
  const [failoverInProgress, setFailoverInProgress] = useState({});

  useEffect(() => {
    fetchGlobalClusters();
  }, [region]);

  const fetchGlobalClusters = async () => {
    setLoading(true);
    try {
      const response = await fetch(`/api/rds/global-clusters?region=${region}`);
      const data = await response.json();
      setRowData(data);
      setLoading(false);
    } catch (error) {
      setMessage(`Failed to fetch global clusters: ${error.message}`);
      setMessageType('danger');
      setLoading(false);
    }
  };

  const handleFailover = async (clusterId) => {
    setFailoverInProgress({ ...failoverInProgress, [clusterId]: true });
    try {
      await fetch(`/api/rds/failover?region=${region}&clusterId=${clusterId}`, { method: 'POST' });
      setMessage(`Failover initiated for cluster: ${clusterId}`);
      setMessageType('success');
      pollFailoverStatus(clusterId);
    } catch (error) {
      setMessage(`Failed to initiate failover: ${error.message}`);
      setMessageType('danger');
      setFailoverInProgress({ ...failoverInProgress, [clusterId]: false });
    }
  };

  const handleFailback = async (clusterId) => {
    setFailoverInProgress({ ...failoverInProgress, [clusterId]: true });
    try {
      await fetch(`/api/rds/failback?region=${region}&clusterId=${clusterId}`, { method: 'POST' });
      setMessage(`Failback initiated for cluster: ${clusterId}`);
      setMessageType('success');
      pollFailoverStatus(clusterId);
    } catch (error) {
      setMessage(`Failed to initiate failback: ${error.message}`);
      setMessageType('danger');
      setFailoverInProgress({ ...failoverInProgress, [clusterId]: false });
    }
  };

  const pollFailoverStatus = async (clusterId) => {
    const interval = setInterval(async () => {
      try {
        const response = await fetch(`/api/rds/failover-status?region=${region}&clusterId=${clusterId}`);
        const data = await response.json();
        if (data.status === 'completed') {
          clearInterval(interval);
          setFailoverInProgress({ ...failoverInProgress, [clusterId]: false });
          fetchGlobalClusters();
        }
      } catch (error) {
        clearInterval(interval);
        setMessage(`Failed to fetch failover status: ${error.message}`);
        setMessageType('danger');
        setFailoverInProgress({ ...failoverInProgress, [clusterId]: false });
      }
    }, 5000);
  };

  const columnDefs = [
    { headerName: 'Global Cluster', field: 'globalClusterId', filter: true, sortable: true },
    { headerName: 'Primary Region', field: 'primaryRegion', filter: true, sortable: true },
    { headerName: 'No Secondary Regions', field: 'secondaryRegionCount', filter: true, sortable: true },
    { headerName: 'Secondary Region Names', field: 'secondaryRegionNames', filter: true, sortable: true },
    {
      headerName: 'Actions',
      field: 'actions',
      cellRendererFramework: (params) => (
        <>
          <CButton color="danger" onClick={() => handleFailover(params.data.globalClusterId)} disabled={failoverInProgress[params.data.globalClusterId]}>
            {failoverInProgress[params.data.globalClusterId] ? <CSpinner size="sm" /> : 'Failover'}
          </CButton>{' '}
          <CButton color="warning" onClick={() => handleFailback(params.data.globalClusterId)} disabled={failoverInProgress[params.data.globalClusterId]}>
            {failoverInProgress[params.data.globalClusterId] ? <CSpinner size="sm" /> : 'Failback'}
          </CButton>
        </>
      ),
    },
  ];

  const defaultColDef = {
    sortable: true,
    filter: true,
    resizable: true,
    enableRowGroup: true,
  };

  return (
    <div>
      <h2>Global RDS Clusters</h2>
      <RegionDropdown selectedRegion={region} onChange={(e) => setRegion(e.target.value)} />
      {message && <CAlert color={messageType}>{message}</CAlert>}
      {loading ? (
        <CSpinner color="primary" />
      ) : (
        <div className="ag-theme-alpine" style={{ height: 600, width: '100%' }}>
          <AgGridReact
            columnDefs={columnDefs}
            rowData={rowData}
            defaultColDef={defaultColDef}
            pagination={true}
            paginationPageSize={10}
            domLayout="autoHeight"
          />
        </div>
      )}
    </div>
  );
};

export default GlobalRdsClusters;
