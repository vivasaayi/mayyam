// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


import React, { useState, useEffect, useRef } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CModal,
  CModalHeader,
  CModalTitle,
  CModalBody,
  CModalFooter,
  CForm,
  CFormInput,
  CFormLabel,
  CFormSelect,
  CFormTextarea,
  CAlert,
  CSpinner,
  CBadge,
  CProgress,
  CProgressBar,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CDropdown,
  CDropdownToggle,
  CDropdownMenu,
  CDropdownItem
} from "@coreui/react";
import { AgGridReact } from "ag-grid-react";
import { CChart } from "@coreui/react-chartjs";
import PageHeader from "../components/layout/PageHeader";
import {
  getAuroraClusters,
  getAuroraCluster,
  createAuroraCluster,
  updateAuroraCluster,
  deleteAuroraCluster,
  testAuroraClusterConnection
} from "../services/api";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";

const AuroraClusters = () => {
  // State management
  const [clusters, setClusters] = useState([]);
  const [selectedCluster, setSelectedCluster] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Modal state
  const [showClusterModal, setShowClusterModal] = useState(false);
  const [editingCluster, setEditingCluster] = useState(null);
  const [clusterForm, setClusterForm] = useState({
    cluster_identifier: "",
    region: "",
    database_name: "",
    master_username: "",
    master_password: "",
    host: "",
    port: 3306,
    engine_version: "8.0.mysql_aurora.3.02.0",
    instance_class: "db.r5.large",
    multi_az: false,
    backup_retention_period: 7,
    monitoring_interval: 60,
    enhanced_monitoring: true,
    performance_insights: true,
    tags: ""
  });

  // Connection test state
  const [testingConnection, setTestingConnection] = useState(false);
  const [connectionResult, setConnectionResult] = useState(null);

  // Grid ref
  const clustersGridRef = useRef();

  // Load clusters on component mount
  useEffect(() => {
    fetchClusters();
  }, []);

  const fetchClusters = async () => {
    try {
      setLoading(true);
      const response = await getAuroraClusters();
      setClusters(response || []);
    } catch (err) {
      setError("Failed to fetch Aurora clusters: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const resetClusterForm = () => {
    setClusterForm({
      cluster_identifier: "",
      region: "",
      database_name: "",
      master_username: "",
      master_password: "",
      host: "",
      port: 3306,
      engine_version: "8.0.mysql_aurora.3.02.0",
      instance_class: "db.r5.large",
      multi_az: false,
      backup_retention_period: 7,
      monitoring_interval: 60,
      enhanced_monitoring: true,
      performance_insights: true,
      tags: ""
    });
  };

  const handleCreateCluster = async (e) => {
    e.preventDefault();
    try {
      setLoading(true);
      const formData = {
        ...clusterForm,
        tags: clusterForm.tags ? JSON.parse(clusterForm.tags) : {}
      };

      if (editingCluster) {
        await updateAuroraCluster(editingCluster.id, formData);
        setSuccess("Cluster updated successfully");
      } else {
        await createAuroraCluster(formData);
        setSuccess("Cluster created successfully");
      }
      setShowClusterModal(false);
      setEditingCluster(null);
      resetClusterForm();
      await fetchClusters();
    } catch (err) {
      setError("Failed to save cluster: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const handleEditCluster = (cluster) => {
    setEditingCluster(cluster);
    setClusterForm({
      cluster_identifier: cluster.cluster_identifier || "",
      region: cluster.region || "",
      database_name: cluster.database_name || "",
      master_username: cluster.master_username || "",
      master_password: "", // Don't populate password for security
      host: cluster.host || "",
      port: cluster.port || 3306,
      engine_version: cluster.engine_version || "8.0.mysql_aurora.3.02.0",
      instance_class: cluster.instance_class || "db.r5.large",
      multi_az: cluster.multi_az || false,
      backup_retention_period: cluster.backup_retention_period || 7,
      monitoring_interval: cluster.monitoring_interval || 60,
      enhanced_monitoring: cluster.enhanced_monitoring !== false,
      performance_insights: cluster.performance_insights !== false,
      tags: cluster.tags ? JSON.stringify(cluster.tags, null, 2) : ""
    });
    setShowClusterModal(true);
  };

  const handleDeleteCluster = async (clusterId) => {
    if (!window.confirm("Are you sure you want to delete this cluster?")) return;

    try {
      await deleteAuroraCluster(clusterId);
      setSuccess("Cluster deleted successfully");
      await fetchClusters();
      if (selectedCluster?.id === clusterId) {
        setSelectedCluster(null);
      }
    } catch (err) {
      setError("Failed to delete cluster: " + (err.response?.data?.message || err.message));
    }
  };

  const handleTestConnection = async (clusterId) => {
    try {
      setTestingConnection(true);
      setConnectionResult(null);
      const response = await testAuroraClusterConnection(clusterId);
      setConnectionResult({
        success: response.success,
        message: response.message,
        latency_ms: response.latency_ms
      });
    } catch (err) {
      setConnectionResult({
        success: false,
        message: err.response?.data?.message || err.message
      });
    } finally {
      setTestingConnection(false);
    }
  };

  const getStatusBadge = (status) => {
    const statusColors = {
      'available': 'success',
      'creating': 'warning',
      'deleting': 'danger',
      'failed': 'danger',
      'modifying': 'warning',
      'rebooting': 'warning',
      'renaming': 'warning',
      'resetting-master-credentials': 'warning',
      'starting': 'info',
      'stopped': 'secondary',
      'stopping': 'warning'
    };
    return <CBadge color={statusColors[status] || 'secondary'}>{status || 'unknown'}</CBadge>;
  };

  const clusterColumnDefs = [
    {
      headerName: "Cluster Identifier",
      field: "cluster_identifier",
      sortable: true,
      filter: true,
      width: 200
    },
    {
      headerName: "Region",
      field: "region",
      sortable: true,
      filter: true,
      width: 120
    },
    {
      headerName: "Status",
      field: "status",
      sortable: true,
      filter: true,
      width: 120,
      cellRenderer: (params) => getStatusBadge(params.value)
    },
    {
      headerName: "Engine Version",
      field: "engine_version",
      sortable: true,
      filter: true,
      width: 180
    },
    {
      headerName: "Instance Class",
      field: "instance_class",
      sortable: true,
      filter: true,
      width: 140
    },
    {
      headerName: "Multi-AZ",
      field: "multi_az",
      sortable: true,
      filter: true,
      width: 100,
      cellRenderer: (params) => params.value ? "Yes" : "No"
    },
    {
      headerName: "Created",
      field: "created_at",
      sortable: true,
      filter: true,
      width: 160,
      valueFormatter: (params) => {
        if (!params.value) return '';
        return new Date(params.value).toLocaleDateString();
      }
    },
    {
      headerName: "Actions",
      field: "actions",
      width: 200,
      cellRenderer: (params) => (
        <div>
          <CButton
            size="sm"
            color="info"
            variant="outline"
            className="me-1"
            onClick={() => handleTestConnection(params.data.id)}
            disabled={testingConnection}
          >
            {testingConnection ? <CSpinner size="sm" /> : "Test"}
          </CButton>
          <CButton
            size="sm"
            color="warning"
            variant="outline"
            className="me-1"
            onClick={() => handleEditCluster(params.data)}
          >
            Edit
          </CButton>
          <CButton
            size="sm"
            color="danger"
            variant="outline"
            onClick={() => handleDeleteCluster(params.data.id)}
          >
            Delete
          </CButton>
        </div>
      )
    }
  ];

  const awsRegions = [
    { value: "us-east-1", label: "US East (N. Virginia)" },
    { value: "us-east-2", label: "US East (Ohio)" },
    { value: "us-west-1", label: "US West (N. California)" },
    { value: "us-west-2", label: "US West (Oregon)" },
    { value: "eu-west-1", label: "EU (Ireland)" },
    { value: "eu-central-1", label: "EU (Frankfurt)" },
    { value: "ap-southeast-1", label: "Asia Pacific (Singapore)" },
    { value: "ap-northeast-1", label: "Asia Pacific (Tokyo)" }
  ];

  const instanceClasses = [
    "db.t3.small", "db.t3.medium", "db.t3.large",
    "db.r5.large", "db.r5.xlarge", "db.r5.2xlarge", "db.r5.4xlarge",
    "db.r6g.large", "db.r6g.xlarge", "db.r6g.2xlarge", "db.r6g.4xlarge"
  ];

  return (
    <div>
      <PageHeader
        title="Aurora Clusters"
        breadcrumbs={[
          { label: "Aurora Clusters" }
        ]}
      />

      {error && (
        <CAlert color="danger" dismissible onClose={() => setError(null)}>
          {error}
        </CAlert>
      )}

      {success && (
        <CAlert color="success" dismissible onClose={() => setSuccess(null)}>
          {success}
        </CAlert>
      )}

      {connectionResult && (
        <CAlert
          color={connectionResult.success ? "success" : "danger"}
          dismissible
          onClose={() => setConnectionResult(null)}
        >
          <strong>Connection Test Result:</strong> {connectionResult.message}
          {connectionResult.latency_ms && (
            <div>Latency: {connectionResult.latency_ms}ms</div>
          )}
        </CAlert>
      )}

      <CRow>
        <CCol xs={12}>
          <CCard>
            <CCardHeader>
              <div className="d-flex justify-content-between align-items-center">
                <h5 className="mb-0">Aurora Clusters</h5>
                <CButton
                  color="primary"
                  onClick={() => {
                    setEditingCluster(null);
                    resetClusterForm();
                    setShowClusterModal(true);
                  }}
                >
                  Add Cluster
                </CButton>
              </div>
            </CCardHeader>
            <CCardBody>
              {loading ? (
                <div className="text-center">
                  <CSpinner />
                  <div className="mt-2">Loading clusters...</div>
                </div>
              ) : (
                <div className="ag-theme-alpine" style={{ height: '600px', width: '100%' }}>
                  <AgGridReact
                    ref={clustersGridRef}
                    rowData={clusters}
                    columnDefs={clusterColumnDefs}
                    pagination={true}
                    paginationPageSize={20}
                    enableSorting={true}
                    enableFilter={true}
                    enableColResize={true}
                    defaultColDef={{
                      resizable: true,
                      sortable: true,
                      filter: true
                    }}
                  />
                </div>
              )}
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      {/* Cluster Modal */}
      <CModal size="lg" visible={showClusterModal} onClose={() => setShowClusterModal(false)}>
        <CModalHeader>
          <CModalTitle>{editingCluster ? "Edit Cluster" : "Add Aurora Cluster"}</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <CForm onSubmit={handleCreateCluster}>
            <CRow>
              <CCol md={6}>
                <CFormLabel>Cluster Identifier</CFormLabel>
                <CFormInput
                  type="text"
                  value={clusterForm.cluster_identifier}
                  onChange={(e) => setClusterForm({...clusterForm, cluster_identifier: e.target.value})}
                  required
                />
              </CCol>
              <CCol md={6}>
                <CFormLabel>Region</CFormLabel>
                <CFormSelect
                  value={clusterForm.region}
                  onChange={(e) => setClusterForm({...clusterForm, region: e.target.value})}
                  required
                >
                  <option value="">Select Region</option>
                  {awsRegions.map(region => (
                    <option key={region.value} value={region.value}>{region.label}</option>
                  ))}
                </CFormSelect>
              </CCol>
            </CRow>

            <CRow className="mt-3">
              <CCol md={6}>
                <CFormLabel>Database Name</CFormLabel>
                <CFormInput
                  type="text"
                  value={clusterForm.database_name}
                  onChange={(e) => setClusterForm({...clusterForm, database_name: e.target.value})}
                />
              </CCol>
              <CCol md={6}>
                <CFormLabel>Master Username</CFormLabel>
                <CFormInput
                  type="text"
                  value={clusterForm.master_username}
                  onChange={(e) => setClusterForm({...clusterForm, master_username: e.target.value})}
                  required
                />
              </CCol>
            </CRow>

            <CRow className="mt-3">
              <CCol md={6}>
                <CFormLabel>Master Password</CFormLabel>
                <CFormInput
                  type="password"
                  value={clusterForm.master_password}
                  onChange={(e) => setClusterForm({...clusterForm, master_password: e.target.value})}
                  required={!editingCluster}
                  placeholder={editingCluster ? "Leave blank to keep current password" : ""}
                />
              </CCol>
              <CCol md={3}>
                <CFormLabel>Host</CFormLabel>
                <CFormInput
                  type="text"
                  value={clusterForm.host}
                  onChange={(e) => setClusterForm({...clusterForm, host: e.target.value})}
                  placeholder="Auto-detected from cluster"
                />
              </CCol>
              <CCol md={3}>
                <CFormLabel>Port</CFormLabel>
                <CFormInput
                  type="number"
                  value={clusterForm.port}
                  onChange={(e) => setClusterForm({...clusterForm, port: parseInt(e.target.value)})}
                />
              </CCol>
            </CRow>

            <CRow className="mt-3">
              <CCol md={6}>
                <CFormLabel>Engine Version</CFormLabel>
                <CFormInput
                  type="text"
                  value={clusterForm.engine_version}
                  onChange={(e) => setClusterForm({...clusterForm, engine_version: e.target.value})}
                />
              </CCol>
              <CCol md={6}>
                <CFormLabel>Instance Class</CFormLabel>
                <CFormSelect
                  value={clusterForm.instance_class}
                  onChange={(e) => setClusterForm({...clusterForm, instance_class: e.target.value})}
                >
                  {instanceClasses.map(cls => (
                    <option key={cls} value={cls}>{cls}</option>
                  ))}
                </CFormSelect>
              </CCol>
            </CRow>

            <CRow className="mt-3">
              <CCol md={3}>
                <CFormLabel>Multi-AZ</CFormLabel>
                <CFormSelect
                  value={clusterForm.multi_az}
                  onChange={(e) => setClusterForm({...clusterForm, multi_az: e.target.value === 'true'})}
                >
                  <option value={false}>No</option>
                  <option value={true}>Yes</option>
                </CFormSelect>
              </CCol>
              <CCol md={3}>
                <CFormLabel>Backup Retention (days)</CFormLabel>
                <CFormInput
                  type="number"
                  value={clusterForm.backup_retention_period}
                  onChange={(e) => setClusterForm({...clusterForm, backup_retention_period: parseInt(e.target.value)})}
                />
              </CCol>
              <CCol md={3}>
                <CFormLabel>Monitoring Interval (seconds)</CFormLabel>
                <CFormInput
                  type="number"
                  value={clusterForm.monitoring_interval}
                  onChange={(e) => setClusterForm({...clusterForm, monitoring_interval: parseInt(e.target.value)})}
                />
              </CCol>
              <CCol md={3}>
                <CFormLabel>Enhanced Monitoring</CFormLabel>
                <CFormSelect
                  value={clusterForm.enhanced_monitoring}
                  onChange={(e) => setClusterForm({...clusterForm, enhanced_monitoring: e.target.value === 'true'})}
                >
                  <option value={true}>Enabled</option>
                  <option value={false}>Disabled</option>
                </CFormSelect>
              </CCol>
            </CRow>

            <CRow className="mt-3">
              <CCol md={6}>
                <CFormLabel>Performance Insights</CFormLabel>
                <CFormSelect
                  value={clusterForm.performance_insights}
                  onChange={(e) => setClusterForm({...clusterForm, performance_insights: e.target.value === 'true'})}
                >
                  <option value={true}>Enabled</option>
                  <option value={false}>Disabled</option>
                </CFormSelect>
              </CCol>
            </CRow>

            <CRow className="mt-3">
              <CCol xs={12}>
                <CFormLabel>Tags (JSON)</CFormLabel>
                <CFormTextarea
                  rows={3}
                  value={clusterForm.tags}
                  onChange={(e) => setClusterForm({...clusterForm, tags: e.target.value})}
                  placeholder='{"Environment": "production", "Team": "backend"}'
                />
              </CCol>
            </CRow>
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowClusterModal(false)}>
            Cancel
          </CButton>
          <CButton
            color="primary"
            onClick={handleCreateCluster}
            disabled={loading}
          >
            {loading ? <CSpinner size="sm" /> : null}
            {editingCluster ? "Update Cluster" : "Create Cluster"}
          </CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default AuroraClusters;