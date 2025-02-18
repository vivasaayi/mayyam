import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { CButton, CSpinner, CProgress } from '@coreui/react';
import RegionDropdown from '../RegionDropdown';

const GlobalClusterStatus = () => {
  const [clusters, setClusters] = useState([]);
  const [selectedRegion, setSelectedRegion] = useState('us-west-2');
  const [loading, setLoading] = useState(false);
  const [failoverStatus, setFailoverStatus] = useState({});

  useEffect(() => {
    fetchClusters();
  }, [selectedRegion]);

  const fetchClusters = async () => {
    setLoading(true);
    try {
      const response = await fetch(`/api/rds/global-clusters-with-replication-flows?region=${selectedRegion}`);
      const data = await response.json();
      setClusters(data);
      setLoading(false);
    } catch (error) {
      console.error("Failed to fetch clusters:", error);
      setLoading(false);
    }
  };

  const handleFailover = async (clusterId, targetRegion, targetDbClusterIdentifier) => {
    setFailoverStatus(prevStatus => ({ ...prevStatus, [clusterId]: 'in-progress' }));
    try {
      await fetch(`/api/rds/failover?region=${selectedRegion}&clusterId=${clusterId}&targetRegion=${targetRegion}&targetDbClusterIdentifier=${targetDbClusterIdentifier}`, {
        method: 'POST',
      });
      alert(`Failover initiated to ${targetRegion}`);
      pollFailoverStatus(clusterId);
    } catch (error) {
      console.error("Failed to initiate failover:", error);
      alert("Failed to initiate failover");
      setFailoverStatus(prevStatus => ({ ...prevStatus, [clusterId]: 'failed' }));
    }
  };

  const handleFailback = async (clusterId) => {
    setFailoverStatus(prevStatus => ({ ...prevStatus, [clusterId]: 'in-progress' }));
    try {
      await fetch(`/api/rds/failback?region=${selectedRegion}&clusterId=${clusterId}`, {
        method: 'POST',
      });
      alert(`Failback initiated`);
      pollFailoverStatus(clusterId);
    } catch (error) {
      console.error("Failed to initiate failback:", error);
      alert("Failed to initiate failback");
      setFailoverStatus(prevStatus => ({ ...prevStatus, [clusterId]: 'failed' }));
    }
  };

  const pollFailoverStatus = async (clusterId) => {
    const interval = setInterval(async () => {
      try {
        const response = await fetch(`/api/rds/failover-status?region=${selectedRegion}&clusterId=${clusterId}`);
        const data = await response.json();
        if (data.status !== 'failing-over') {
          clearInterval(interval);
          setFailoverStatus(prevStatus => ({ ...prevStatus, [clusterId]: 'completed' }));
        }
      } catch (error) {
        console.error("Failed to fetch failover status:", error);
        clearInterval(interval);
        setFailoverStatus(prevStatus => ({ ...prevStatus, [clusterId]: 'failed' }));
      }
    }, 5000);
  };

  const columns = [
    { headerName: "Global Cluster", field: "globalClusterId" },
    { headerName: "Primary Region", field: "region" },
    { headerName: "No Secondary Regions", field: "replicationFlows.length" },
    { headerName: "Secondary Region Names", field: "replicationFlows", valueGetter: params => params.data.replicationFlows.map(flow => flow.targetRegion).join(', ') },
    { headerName: "Status", field: "status" },
    {
      headerName: "Actions",
      field: "actions",
      cellRendererFramework: params => (
        <div>
          {failoverStatus[params.data.globalClusterId] === 'in-progress' ? (
            <CProgress animated value={100} />
          ) : (
            <>
              {params.data.replicationFlows.map((flow, index) => (
                <CButton
                  key={index}
                  color="primary"
                  onClick={() => handleFailover(params.data.globalClusterId, flow.targetRegion, flow.targetArn)}
                >
                  Failover {flow.targetArn.split(':').pop()} to {flow.targetRegion}
                </CButton>
              ))}
              <CButton color="secondary" onClick={() => handleFailback(params.data.globalClusterId)}>Failback</CButton>
            </>
          )}
        </div>
      )
    }
  ];

  const getRowStyle = params => {
    if (params.data.status === 'failing-over' || params.data.status === 'switching-over') {
      return { backgroundColor: 'orange' };
    }
    return null;
  };

  return (
    <div>
      <h2>Global Cluster Status</h2>
      <RegionDropdown selectedRegion={selectedRegion} onChange={(e) => setSelectedRegion(e.target.value)} />
      {loading ? (
        <CSpinner color="primary" />
      ) : (
        <div className="ag-theme-alpine" style={{ height: 400, width: '100%' }}>
          <AgGridReact
            rowData={clusters}
            columnDefs={columns}
            defaultColDef={{ flex: 1, minWidth: 150, resizable: true }}
            getRowStyle={getRowStyle}
          />
        </div>
      )}
    </div>
  );
};

export default GlobalClusterStatus;
