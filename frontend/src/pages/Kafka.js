import React, { useState, useEffect } from "react";
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
  CAlert,
  CSpinner,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CBadge
} from "@coreui/react";
import { FaStream, FaServer, FaDatabase, FaSync } from "react-icons/fa";
import { fetchWithAuth } from "../services/api";

const Kafka = () => {
  const [showAddClusterModal, setShowAddClusterModal] = useState(false);
  const [clusters, setClusters] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);
  const [activeTab, setActiveTab] = useState('clusters');
  const [selectedCluster, setSelectedCluster] = useState(null);
  const [topics, setTopics] = useState([]);
  const [topicDetails, setTopicDetails] = useState(null);

  const handleAddCluster = (e) => {
    e.preventDefault();
    // In a real app, this would call an API
    setClusters([...clusters, {
      id: Date.now().toString(),
      name: e.target.clusterName.value,
      bootstrapServers: e.target.bootstrapServers.value,
      securityProtocol: e.target.securityProtocol.value
    }]);
    setShowAddClusterModal(false);
  };

  // Load clusters from API
  const loadClusters = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetchWithAuth('/api/kafka/clusters');
      const data = await response.json();
      setClusters(data.clusters || []);
    } catch (err) {
      setError(`Failed to load clusters: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Create new cluster
  const createCluster = async (clusterData) => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetchWithAuth('/api/kafka/clusters', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(clusterData)
      });
      setSuccess('Cluster created successfully!');
      loadClusters(); // Reload clusters
      setShowAddClusterModal(false);
    } catch (err) {
      setError(`Failed to create cluster: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Load topics for selected cluster
  const loadTopics = async (clusterId) => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetchWithAuth(`/api/kafka/clusters/${clusterId}/topics`);
      const data = await response.json();
      setTopics(data.topics || data || []);
    } catch (err) {
      setError(`Failed to load topics: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Load topic details
  const loadTopicDetails = async (clusterId, topicName) => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetchWithAuth(`/api/kafka/clusters/${clusterId}/topics/${topicName}`);
      const data = await response.json();
      setTopicDetails(data);
    } catch (err) {
      setError(`Failed to load topic details: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Handle cluster selection
  const handleClusterSelect = (cluster) => {
    setSelectedCluster(cluster);
    setActiveTab('topics');
    loadTopics(cluster.id);
  };

  // Handle topic selection
  const handleTopicSelect = (topic) => {
    loadTopicDetails(selectedCluster.id, topic.name);
  };

  // Clear messages
  const clearMessages = () => {
    setError(null);
    setSuccess(null);
  };

  // Load clusters on component mount
  useEffect(() => {
    loadClusters();
  }, []);

  return (
    <>
      <div className="d-flex justify-content-between align-items-center mb-4">
        <h2><FaServer className="me-2" />Kafka Management</h2>
        <CButton color="primary" onClick={() => setShowAddClusterModal(true)}>
          Add Kafka Cluster
        </CButton>
      </div>

      {/* Alert Messages */}
      {error && (
        <CAlert color="danger" dismissible onClose={clearMessages}>
          {error}
        </CAlert>
      )}
      {success && (
        <CAlert color="success" dismissible onClose={clearMessages}>
          {success}
        </CAlert>
      )}

      <CRow>
        <CCol>
          <CCard>
            <CCardHeader>
              <CNav variant="tabs">
                <CNavItem>
                  <CNavLink
                    active={activeTab === 'clusters'}
                    onClick={() => setActiveTab('clusters')}
                    style={{ cursor: 'pointer' }}
                  >
                    <FaServer className="me-2" />Clusters
                  </CNavLink>
                </CNavItem>
                {selectedCluster && (
                  <CNavItem>
                    <CNavLink
                      active={activeTab === 'topics'}
                      onClick={() => setActiveTab('topics')}
                      style={{ cursor: 'pointer' }}
                    >
                      <FaStream className="me-2" />Topics
                    </CNavLink>
                  </CNavItem>
                )}
              </CNav>
            </CCardHeader>
            <CCardBody>
              <CTabContent>
                {/* Clusters Tab */}
                <CTabPane visible={activeTab === 'clusters'}>
                  {loading ? (
                    <div className="text-center">
                      <CSpinner />
                      <p>Loading clusters...</p>
                    </div>
                  ) : clusters.length === 0 ? (
                    <p>No Kafka clusters connected yet. Use the button above to add your first Kafka connection.</p>
                  ) : (
                    <CRow>
                      {clusters.map(cluster => (
                        <CCol sm={6} lg={4} key={cluster.id} className="mb-4">
                          <CCard
                            style={{
                              cursor: 'pointer',
                              border: selectedCluster?.id === cluster.id ? '2px solid #007bff' : '1px solid #dee2e6'
                            }}
                            onClick={() => handleClusterSelect(cluster)}
                          >
                            <CCardHeader>
                              <strong>{cluster.name}</strong>
                              {selectedCluster?.id === cluster.id && (
                                <CBadge color="primary" className="ms-2">Selected</CBadge>
                              )}
                            </CCardHeader>
                            <CCardBody>
                              <p><strong>Bootstrap Servers:</strong> {cluster.bootstrap_servers?.join(', ') || 'N/A'}</p>
                              <p><strong>Status:</strong> <CBadge color="success">Connected</CBadge></p>
                              <div className="d-flex mt-3">
                                <CButton color="primary" size="sm" className="me-2">Manage Topics</CButton>
                                <CButton color="secondary" size="sm">View Details</CButton>
                              </div>
                            </CCardBody>
                          </CCard>
                        </CCol>
                      ))}
                    </CRow>
                  )}
                </CTabPane>

                {/* Topics Tab */}
                <CTabPane visible={activeTab === 'topics'}>
                  {selectedCluster && (
                    <>
                      <div className="mb-3">
                        <h5>Topics in {selectedCluster.name}</h5>
                        <CButton color="success" size="sm" onClick={() => loadTopics(selectedCluster.id)}>
                          <FaSync className="me-2" />Refresh
                        </CButton>
                      </div>

                      {loading ? (
                        <div className="text-center">
                          <CSpinner />
                          <p>Loading topics...</p>
                        </div>
                      ) : topics.length === 0 ? (
                        <p>No topics found in this cluster.</p>
                      ) : (
                        <CTable responsive hover>
                          <CTableHead>
                            <CTableRow>
                              <CTableHeaderCell>Topic Name</CTableHeaderCell>
                              <CTableHeaderCell>Partitions</CTableHeaderCell>
                              <CTableHeaderCell>Actions</CTableHeaderCell>
                            </CTableRow>
                          </CTableHead>
                          <CTableBody>
                            {topics.map((topic, index) => (
                              <CTableRow key={index}>
                                <CTableDataCell>
                                  <strong>{topic.name}</strong>
                                </CTableDataCell>
                                <CTableDataCell>{topic.partitions}</CTableDataCell>
                                <CTableDataCell>
                                  <CButton
                                    color="info"
                                    size="sm"
                                    onClick={() => handleTopicSelect(topic)}
                                  >
                                    View Details
                                  </CButton>
                                </CTableDataCell>
                              </CTableRow>
                            ))}
                          </CTableBody>
                        </CTable>
                      )}
                    </>
                  )}
                </CTabPane>
              </CTabContent>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      {/* Topic Details Modal */}
      {topicDetails && (
        <CModal visible={true} onClose={() => setTopicDetails(null)} size="lg">
          <CModalHeader>
            <CModalTitle>Topic Details: {topicDetails.name}</CModalTitle>
          </CModalHeader>
          <CModalBody>
            <CRow>
              <CCol md={6}>
                <h6>Partitions</h6>
                {topicDetails.partitions?.map((partition, index) => (
                  <CCard key={index} className="mb-2">
                    <CCardBody className="p-2">
                      <strong>Partition {partition.id}</strong>
                      <br />
                      <small>Leader: {partition.leader}, Replicas: {partition.replicas?.join(', ')}</small>
                      <br />
                      <small>Offsets - Earliest: {partition.offsets?.earliest}, Latest: {partition.offsets?.latest}</small>
                    </CCardBody>
                  </CCard>
                ))}
              </CCol>
              <CCol md={6}>
                <h6>Configuration</h6>
                <pre style={{ fontSize: '12px', maxHeight: '300px', overflow: 'auto' }}>
                  {JSON.stringify(topicDetails.configs, null, 2)}
                </pre>
              </CCol>
            </CRow>
          </CModalBody>
          <CModalFooter>
            <CButton color="secondary" onClick={() => setTopicDetails(null)}>
              Close
            </CButton>
          </CModalFooter>
        </CModal>
      )}

      {/* Add Cluster Modal */}
      <CModal visible={showAddClusterModal} onClose={() => setShowAddClusterModal(false)}>
        <CModalHeader>
          <CModalTitle>Add Kafka Cluster</CModalTitle>
        </CModalHeader>
        <CForm onSubmit={(e) => {
          e.preventDefault();
          const formData = new FormData(e.target);
          const clusterData = {
            name: formData.get('clusterName'),
            bootstrap_servers: [formData.get('bootstrapServers')],
            security_protocol: formData.get('securityProtocol'),
            sasl_username: formData.get('saslUsername') || null,
            sasl_password: formData.get('saslPassword') || null,
            sasl_mechanism: formData.get('saslMechanism') || null
          };
          createCluster(clusterData);
        }}>
          <CModalBody>
            <div className="mb-3">
              <CFormLabel htmlFor="clusterName">Cluster Name</CFormLabel>
              <CFormInput id="clusterName" name="clusterName" placeholder="My Kafka Cluster" required />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="bootstrapServers">Bootstrap Servers</CFormLabel>
              <CFormInput id="bootstrapServers" name="bootstrapServers" placeholder="localhost:9092" required />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="securityProtocol">Security Protocol</CFormLabel>
              <CFormSelect id="securityProtocol" name="securityProtocol">
                <option value="PLAINTEXT">PLAINTEXT</option>
                <option value="SSL">SSL</option>
                <option value="SASL_PLAINTEXT">SASL_PLAINTEXT</option>
                <option value="SASL_SSL">SASL_SSL</option>
              </CFormSelect>
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="saslUsername">SASL Username (optional)</CFormLabel>
              <CFormInput id="saslUsername" name="saslUsername" placeholder="username" />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="saslPassword">SASL Password (optional)</CFormLabel>
              <CFormInput id="saslPassword" name="saslPassword" type="password" placeholder="password" />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="saslMechanism">SASL Mechanism (optional)</CFormLabel>
              <CFormSelect id="saslMechanism" name="saslMechanism">
                <option value="">None</option>
                <option value="PLAIN">PLAIN</option>
                <option value="SCRAM-SHA-256">SCRAM-SHA-256</option>
                <option value="SCRAM-SHA-512">SCRAM-SHA-512</option>
              </CFormSelect>
            </div>
          </CModalBody>
          <CModalFooter>
            <CButton color="secondary" onClick={() => setShowAddClusterModal(false)}>
              Cancel
            </CButton>
            <CButton color="primary" type="submit" disabled={loading}>
              {loading ? <CSpinner size="sm" className="me-2" /> : null}
              Add Cluster
            </CButton>
          </CModalFooter>
        </CForm>
      </CModal>
    </>
  );
};

export default Kafka;
