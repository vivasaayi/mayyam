import React, { useEffect, useState } from 'react'
import axios from 'axios'
import { AgGridReact } from 'ag-grid-react'
import 'ag-grid-community/styles/ag-grid.css'
import 'ag-grid-community/styles/ag-theme-alpine.css'
import { CButton, CModal, CModalBody, CModalFooter, CModalHeader, CModalTitle, CForm, CFormLabel, CFormInput, CFormTextarea } from '@coreui/react'
import yaml from 'js-yaml'
import { ClientSideRowModelModule, DateFilterModule, ModuleRegistry, NumberFilterModule, TextFilterModule, ValidationModule } from 'ag-grid-community';

ModuleRegistry.registerModules([ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]);

const ElastiCacheClusters = () => {
  const [clusters, setClusters] = useState([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)
  const [showModal, setShowModal] = useState(false)
  const [yamlContent, setYamlContent] = useState('')
  const [jsonContent, setJsonContent] = useState({})
  const [formData, setFormData] = useState({})

  useEffect(() => {
    axios.get('/api/elasticache/clusters')
      .then(response => {
        setClusters(response.data)
        setLoading(false)
      })
      .catch(error => {
        console.error('There was an error fetching the ElastiCache clusters!', error)
        setError('There was an error fetching the ElastiCache clusters.')
        setLoading(false)
      })
  }, [])

  const handleYamlChange = (e) => {
    const yamlText = e.target.value
    setYamlContent(yamlText)
    try {
      const json = yaml.load(yamlText)
      setJsonContent(json)
      setFormData(json)
    } catch (err) {
      setJsonContent({ error: 'Invalid YAML' })
    }
  }

  const handleJsonChange = (e) => {
    const jsonText = e.target.value
    try {
      const json = JSON.parse(jsonText)
      setJsonContent(json)
      setFormData(json)
    } catch (err) {
      setJsonContent({ error: 'Invalid JSON' })
    }
  }

  const handleFormChange = (e) => {
    const { name, value } = e.target
    const updatedFormData = {
      ...formData,
      [name]: value,
    }
    setFormData(updatedFormData)
    setYamlContent(yaml.dump(updatedFormData))
    setJsonContent(updatedFormData)
  }

  const handleCreateCluster = () => {
    axios.post('/api/elasticache/create-cluster', formData)
      .then(response => {
        setShowModal(false)
        setClusters([...clusters, response.data])
      })
      .catch(error => {
        console.error('There was an error creating the ElastiCache cluster!', error)
      })
  }

  const sampleYaml = `
cacheClusterId: my-cluster
cacheNodeType: cache.t2.micro
engine: redis
numCacheNodes: 1
engineVersion: "6.x"
cacheParameterGroupName: default.redis6.x
cacheSubnetGroupName: my-subnet-group
securityGroupIds:
  - sg-0123456789abcdef0
tags:
  - Key: Name
    Value: my-cluster
snapshotArns: []
preferredMaintenanceWindow: sun:05:00-sun:09:00
port: 6379
notificationTopicArn: arn:aws:sns:us-west-2:123456789012:my-sns-topic
autoMinorVersionUpgrade: true
snapshotName: my-snapshot
preferredAvailabilityZone: us-west-2a
preferredAvailabilityZones: []
replicationGroupId: my-replication-group
azMode: single-az
`

  useEffect(() => {
    setYamlContent(sampleYaml)
    try {
      const json = yaml.load(sampleYaml)
      setJsonContent(json)
      setFormData(json)
    } catch (err) {
      setJsonContent({ error: 'Invalid YAML' })
    }
  }, [])

  if (loading) {
    return <div>Loading...</div>
  }

  if (error) {
    return <div>{error}</div>
  }

  const columnDefs = [
    { headerName: 'Cluster ID', field: 'cacheClusterId' },
    { headerName: 'Node Type', field: 'cacheNodeType' },
    { headerName: 'Engine', field: 'engine' },
    { headerName: 'Status', field: 'cacheClusterStatus' },
    { headerName: 'Num Nodes', field: 'numCacheNodes' },
  ]

  return (
    <div>
      <h1>ElastiCache Clusters</h1>
      <CButton color="primary" onClick={() => setShowModal(true)}>Create ElastiCache</CButton>
      <div className="ag-theme-alpine" style={{ height: 400, width: '100%' }}>
        <AgGridReact
          rowData={clusters}
          columnDefs={columnDefs}
          pagination={true}
          paginationPageSize={10}
          defaultColDef={{ sortable: true, filter: true, resizable: true }}
          sideBar={{ toolPanels: ['columns'] }}
        />
      </div>

      <CModal visible={showModal} onClose={() => setShowModal(false)} size="lg">
        <CModalHeader onClose={() => setShowModal(false)}>
          <CModalTitle>Create ElastiCache</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <div style={{ display: 'flex' }}>
            <CFormTextarea
              value={yamlContent}
              onChange={handleYamlChange}
              placeholder="Paste YAML here"
              style={{ width: '50%', height: '300px', marginRight: '10px' }}
            />
            <CFormTextarea
              value={JSON.stringify(jsonContent, null, 2)}
              onChange={handleJsonChange}
              placeholder="JSON will appear here"
              style={{ width: '50%', height: '300px' }}
            />
          </div>
          <CForm>
            <div className="mb-3">
              <CFormLabel htmlFor="cacheClusterId">Cluster ID</CFormLabel>
              <CFormInput
                type="text"
                id="cacheClusterId"
                name="cacheClusterId"
                value={formData.cacheClusterId || ''}
                onChange={handleFormChange}
                placeholder="Enter Cluster ID"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="cacheNodeType">Node Type</CFormLabel>
              <CFormInput
                type="text"
                id="cacheNodeType"
                name="cacheNodeType"
                value={formData.cacheNodeType || ''}
                onChange={handleFormChange}
                placeholder="Enter Node Type"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="engine">Engine</CFormLabel>
              <CFormInput
                type="text"
                id="engine"
                name="engine"
                value={formData.engine || ''}
                onChange={handleFormChange}
                placeholder="Enter Engine"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="numCacheNodes">Number of Nodes</CFormLabel>
              <CFormInput
                type="number"
                id="numCacheNodes"
                name="numCacheNodes"
                value={formData.numCacheNodes || ''}
                onChange={handleFormChange}
                placeholder="Enter Number of Nodes"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="engineVersion">Engine Version</CFormLabel>
              <CFormInput
                type="text"
                id="engineVersion"
                name="engineVersion"
                value={formData.engineVersion || ''}
                onChange={handleFormChange}
                placeholder="Enter Engine Version"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="cacheParameterGroupName">Cache Parameter Group Name</CFormLabel>
              <CFormInput
                type="text"
                id="cacheParameterGroupName"
                name="cacheParameterGroupName"
                value={formData.cacheParameterGroupName || ''}
                onChange={handleFormChange}
                placeholder="Enter Cache Parameter Group Name"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="cacheSubnetGroupName">Cache Subnet Group Name</CFormLabel>
              <CFormInput
                type="text"
                id="cacheSubnetGroupName"
                name="cacheSubnetGroupName"
                value={formData.cacheSubnetGroupName || ''}
                onChange={handleFormChange}
                placeholder="Enter Cache Subnet Group Name"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="securityGroupIds">Security Group IDs</CFormLabel>
              <CFormInput
                type="text"
                id="securityGroupIds"
                name="securityGroupIds"
                value={formData.securityGroupIds || ''}
                onChange={handleFormChange}
                placeholder="Enter Security Group IDs (comma separated)"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="tags">Tags</CFormLabel>
              <CFormInput
                type="text"
                id="tags"
                name="tags"
                value={formData.tags || ''}
                onChange={handleFormChange}
                placeholder="Enter Tags (comma separated key=value pairs)"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="snapshotArns">Snapshot ARNs</CFormLabel>
              <CFormInput
                type="text"
                id="snapshotArns"
                name="snapshotArns"
                value={formData.snapshotArns || ''}
                onChange={handleFormChange}
                placeholder="Enter Snapshot ARNs (comma separated)"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="preferredMaintenanceWindow">Preferred Maintenance Window</CFormLabel>
              <CFormInput
                type="text"
                id="preferredMaintenanceWindow"
                name="preferredMaintenanceWindow"
                value={formData.preferredMaintenanceWindow || ''}
                onChange={handleFormChange}
                placeholder="Enter Preferred Maintenance Window"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="port">Port</CFormLabel>
              <CFormInput
                type="number"
                id="port"
                name="port"
                value={formData.port || ''}
                onChange={handleFormChange}
                placeholder="Enter Port"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="notificationTopicArn">Notification Topic ARN</CFormLabel>
              <CFormInput
                type="text"
                id="notificationTopicArn"
                name="notificationTopicArn"
                value={formData.notificationTopicArn || ''}
                onChange={handleFormChange}
                placeholder="Enter Notification Topic ARN"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="autoMinorVersionUpgrade">Auto Minor Version Upgrade</CFormLabel>
              <CFormInput
                type="checkbox"
                id="autoMinorVersionUpgrade"
                name="autoMinorVersionUpgrade"
                checked={formData.autoMinorVersionUpgrade || false}
                onChange={handleFormChange}
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="snapshotName">Snapshot Name</CFormLabel>
              <CFormInput
                type="text"
                id="snapshotName"
                name="snapshotName"
                value={formData.snapshotName || ''}
                onChange={handleFormChange}
                placeholder="Enter Snapshot Name"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="preferredAvailabilityZone">Preferred Availability Zone</CFormLabel>
              <CFormInput
                type="text"
                id="preferredAvailabilityZone"
                name="preferredAvailabilityZone"
                value={formData.preferredAvailabilityZone || ''}
                onChange={handleFormChange}
                placeholder="Enter Preferred Availability Zone"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="preferredAvailabilityZones">Preferred Availability Zones</CFormLabel>
              <CFormInput
                type="text"
                id="preferredAvailabilityZones"
                name="preferredAvailabilityZones"
                value={formData.preferredAvailabilityZones || ''}
                onChange={handleFormChange}
                placeholder="Enter Preferred Availability Zones (comma separated)"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="replicationGroupId">Replication Group ID</CFormLabel>
              <CFormInput
                type="text"
                id="replicationGroupId"
                name="replicationGroupId"
                value={formData.replicationGroupId || ''}
                onChange={handleFormChange}
                placeholder="Enter Replication Group ID"
              />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="azMode">AZ Mode</CFormLabel>
              <CFormInput
                type="text"
                id="azMode"
                name="azMode"
                value={formData.azMode || ''}
                onChange={handleFormChange}
                placeholder="Enter AZ Mode"
              />
            </div>
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowModal(false)}>Cancel</CButton>
          <CButton color="primary" onClick={handleCreateCluster}>Create ElastiCache</CButton>
        </CModalFooter>
      </CModal>
    </div>
  )
}

export default ElastiCacheClusters