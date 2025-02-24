import React, { useEffect, useState } from 'react'
import axios from 'axios'
import { AgGridReact } from 'ag-grid-react'
import 'ag-grid-community/styles/ag-grid.css'
import 'ag-grid-community/styles/ag-theme-alpine.css'
import { ClientSideRowModelModule, DateFilterModule, ModuleRegistry, NumberFilterModule, TextFilterModule, ValidationModule } from 'ag-grid-community';

ModuleRegistry.registerModules([ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]);

const ElastiCacheReplicationGroups = () => {
  const [groups, setGroups] = useState([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)

  useEffect(() => {
    axios.get('/api/elasticache/replication-groups')
      .then(response => {
        setGroups(response.data)
        setLoading(false)
      })
      .catch(error => {
        console.error('There was an error fetching the ElastiCache replication groups!', error)
        setError('There was an error fetching the ElastiCache replication groups.')
        setLoading(false)
      })
  }, [])

  if (loading) {
    return <div>Loading...</div>
  }

  if (error) {
    return <div>{error}</div>
  }

  const columnDefs = [
    { headerName: 'Group ID', field: 'replicationGroupId' },
    { headerName: 'Status', field: 'status' },
    { headerName: 'Primary Endpoint', field: 'primaryEndpoint.address' },
    { headerName: 'Node Groups', field: 'nodeGroups.length' },
  ]

  return (
    <div>
      <h1>ElastiCache Replication Groups</h1>
      <div className="ag-theme-alpine" style={{ height: 400, width: '100%' }}>
        <AgGridReact
          rowData={groups}
          columnDefs={columnDefs}
          pagination={true}
          paginationPageSize={10}
          defaultColDef={{ sortable: true, filter: true, resizable: true }}
          sideBar={{ toolPanels: ['columns'] }}
        />
      </div>
    </div>
  )
}

export default ElastiCacheReplicationGroups