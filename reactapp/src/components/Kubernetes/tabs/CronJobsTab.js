import React, { useEffect, useState } from 'react';
import { CButton } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule } from 'ag-grid-community';
import axios from 'axios';

const CronJobsTab = ({ namespace }) => {
  const [cronJobsData, setCronJobsData] = useState([]);
  const [error, setError] = useState(null);

  useEffect(() => {
    fetchCronJobs();
  }, [namespace]);

  const fetchCronJobs = async () => {
    try {
      const response = await axios.get(`/api/kubernetes/cronjobs?namespace=${namespace}`);
      setCronJobsData(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching cron jobs:', error);
      setError('Error fetching cron jobs');
    }
  };

  return (
    <div>
      <CButton color="primary" onClick={fetchCronJobs}>Load</CButton>
      {error && <div className="alert alert-danger">{error}</div>}
      <div className="ag-theme-balham" style={{ height: 400, width: '100%', marginTop: '10px' }}>
        <AgGridReact
          rowData={cronJobsData}
          columnDefs={[
            { headerName: 'Name', field: 'name' },
            { headerName: 'Expected Replicas', field: 'expectedReplicas' },
            { headerName: 'Pods Running', field: 'podsRunning' },
            { headerName: 'Pods Pending', field: 'podsPending' },
            { headerName: 'Pods Not Started', field: 'podsNotStarted' },
            { headerName: 'Actions', field: 'actions', cellRendererFramework: (params) => <CButton color="primary">View Pod Details</CButton> },
          ]}
          defaultColDef={{ flex: 1, minWidth: 100 }}
          modules={[ClientSideRowModelModule]}
        />
      </div>
    </div>
  );
};

export default CronJobsTab;
