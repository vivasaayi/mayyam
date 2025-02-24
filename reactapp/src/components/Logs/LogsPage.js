import React, { useState, useEffect } from 'react';
import { CButton, CInput, CFormGroup, CLabel } from '@coreui/react';
import axios from 'axios';

const LogsPage = () => {
  const [logs, setLogs] = useState([]);
  const [filter, setFilter] = useState('');

  useEffect(() => {
    fetchLogs();
  }, []);

  const fetchLogs = async () => {
    try {
      const response = await axios.get('/api/logs');
      setLogs(response.data);
    } catch (error) {
      console.error('Error fetching logs:', error);
    }
  };

  const handleFilterChange = (e) => {
    setFilter(e.target.value);
  };

  const filteredLogs = logs.filter(log => log.message.includes(filter));

  return (
    <div>
      <h2>Logs</h2>
      <CFormGroup>
        <CLabel htmlFor="filter">Filter Logs</CLabel>
        <CInput id="filter" value={filter} onChange={handleFilterChange} />
      </CFormGroup>
      <CButton color="primary" onClick={fetchLogs}>Refresh Logs</CButton>
      <div>
        {filteredLogs.map((log, index) => (
          <div key={index}>
            <pre>{JSON.stringify(log, null, 2)}</pre>
          </div>
        ))}
      </div>
    </div>
  );
};

export default LogsPage;