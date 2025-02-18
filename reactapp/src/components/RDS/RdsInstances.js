import React, { useEffect, useState } from 'react';
import axios from 'axios';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';

const RdsInstances = () => {
  const [instances, setInstances] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [formData, setFormData] = useState({
    dbInstanceIdentifier: '',
    dbInstanceClass: '',
    engine: '',
    allocatedStorage: '',
  });

  useEffect(() => {
    axios.get('/api/rds/instances')
      .then(response => {
        setInstances(response.data);
        setLoading(false);
      })
      .catch(error => {
        console.error('There was an error fetching the RDS instances!', error);
        setError('There was an error fetching the RDS instances.');
        setLoading(false);
      });
  }, []);

  const handleInputChange = (e) => {
    const { name, value } = e.target;
    setFormData({
      ...formData,
      [name]: value,
    });
  };

  const handleSubmit = (e) => {
    e.preventDefault();
    axios.post('/api/rds/create', formData)
      .then(response => {
        setInstances([...instances, response.data]);
        setFormData({
          dbInstanceIdentifier: '',
          dbInstanceClass: '',
          engine: '',
          allocatedStorage: '',
        });
      })
      .catch(error => {
        console.error('There was an error creating the RDS instance!', error);
        setError('There was an error creating the RDS instance.');
      });
  };

  if (loading) {
    return <div>Loading...</div>;
  }

  if (error) {
    return <div>{error}</div>;
  }

  const columnDefs = [
    { headerName: 'DB Instance Identifier', field: 'dbInstanceIdentifier' },
    { headerName: 'DB Instance Class', field: 'dbInstanceClass' },
    { headerName: 'Engine', field: 'engine' },
    { headerName: 'Allocated Storage (GB)', field: 'allocatedStorage' },
    { headerName: 'Status', field: 'dbInstanceStatus' },
    { headerName: 'Endpoint Address', field: 'endpoint.address' },
    { headerName: 'Endpoint Port', field: 'endpoint.port' },
    { headerName: 'Availability Zone', field: 'availabilityZone' },
    { headerName: 'Engine Version', field: 'engineVersion' },
    { headerName: 'Publicly Accessible', field: 'publiclyAccessible' },
    { headerName: 'Storage Type', field: 'storageType' },
    { headerName: 'Multi-AZ', field: 'multiAZ' },
    { headerName: 'Instance Create Time', field: 'instanceCreateTime' },
  ];

  const getRowStyle = params => {
    if (params.data.dbInstanceStatus === 'available') {
      return { backgroundColor: 'lightgreen' };
    }
    return null;
  };

  return (
    <div>
      <h1>RDS Instances</h1>
      <form onSubmit={handleSubmit}>
        <div>
          <label>DB Instance Identifier:</label>
          <input
            type="text"
            name="dbInstanceIdentifier"
            value={formData.dbInstanceIdentifier}
            onChange={handleInputChange}
            required
          />
        </div>
        <div>
          <label>DB Instance Class:</label>
          <input
            type="text"
            name="dbInstanceClass"
            value={formData.dbInstanceClass}
            onChange={handleInputChange}
            required
          />
        </div>
        <div>
          <label>Engine:</label>
          <input
            type="text"
            name="engine"
            value={formData.engine}
            onChange={handleInputChange}
            required
          />
        </div>
        <div>
          <label>Allocated Storage (GB):</label>
          <input
            type="number"
            name="allocatedStorage"
            value={formData.allocatedStorage}
            onChange={handleInputChange}
            required
          />
        </div>
        <button type="submit">Create RDS Instance</button>
      </form>
      <div className="ag-theme-alpine" style={{ height: 400, width: '100%' }}>
        <AgGridReact
          rowData={instances}
          columnDefs={columnDefs}
          pagination={true}
          paginationPageSize={10}
          getRowStyle={getRowStyle}
        />
      </div>
    </div>
  );
};

export default RdsInstances;
