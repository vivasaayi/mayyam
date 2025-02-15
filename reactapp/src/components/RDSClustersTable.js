import React, { useState, useEffect } from 'react';
import { AgGridReact } from 'ag-grid-react';
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';
import { useHistory } from 'react-router-dom';

const RDSClustersTable = () => {
    const history = useHistory();
    const [clusters, setClusters] = useState([]);
    const [selectedClusters, setSelectedClusters] = useState([]);

    useEffect(() => {
        fetch('/api/rds/clusters')
            .then(response => response.json())
            .then(data => setClusters(data))
            .catch(error => console.error('Error fetching RDS clusters:', error));
    }, []);

    const onSelectionChanged = (event) => {
        setSelectedClusters(event.api.getSelectedRows());
    };

    const handleScaleDown = () => {
        if (window.confirm('Are you sure you want to scale down the selected clusters?')) {
            fetch('/api/rds/scale-down', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ clusters: selectedClusters }),
            })
                .then(response => response.json())
                .then(data => {
                    alert('Scale down initiated successfully');
                    // Optionally refresh the table data
                })
                .catch(error => console.error('Error scaling down clusters:', error));
        }
    };

    const columnDefs = [
        { headerName: 'Cluster ID', field: 'clusterId', checkboxSelection: true },
        { headerName: 'Cluster Name', field: 'clusterName' },
        { headerName: 'Status', field: 'status' },
    ];

    return (
        <div className="ag-theme-alpine" style={{ height: 400, width: 600 }}>
            <AgGridReact
                rowData={clusters}
                columnDefs={columnDefs}
                rowSelection="multiple"
                onSelectionChanged={onSelectionChanged}
            />
            <button onClick={handleScaleDown} disabled={selectedClusters.length === 0}>
                Scale Down Selected Clusters
            </button>
            <button onClick={() => history.push('/')}>
                Go Back
            </button>
        </div>
    );
};

export default RDSClustersTable;
