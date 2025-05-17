import React, { useState } from "react";
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
  CFormSelect
} from "@coreui/react";

const Kafka = () => {
  const [showAddClusterModal, setShowAddClusterModal] = useState(false);
  const [clusters, setClusters] = useState([]);

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

  return (
    <>
      <div className="d-flex justify-content-between align-items-center mb-4">
        <h2>Kafka Management</h2>
        <CButton color="primary" onClick={() => setShowAddClusterModal(true)}>
          Add Kafka Cluster
        </CButton>
      </div>

      <CCard className="mb-4">
        <CCardHeader>Connected Kafka Clusters</CCardHeader>
        <CCardBody>
          {clusters.length === 0 ? (
            <p>No Kafka clusters connected yet. Use the button above to add your first Kafka connection.</p>
          ) : (
            <CRow>
              {clusters.map(cluster => (
                <CCol sm={6} lg={4} key={cluster.id} className="mb-4">
                  <CCard>
                    <CCardHeader>{cluster.name}</CCardHeader>
                    <CCardBody>
                      <p><strong>Bootstrap Servers:</strong> {cluster.bootstrapServers}</p>
                      <p><strong>Security Protocol:</strong> {cluster.securityProtocol}</p>
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
        </CCardBody>
      </CCard>

      {/* Add Cluster Modal */}
      <CModal visible={showAddClusterModal} onClose={() => setShowAddClusterModal(false)}>
        <CModalHeader>
          <CModalTitle>Add Kafka Cluster</CModalTitle>
        </CModalHeader>
        <CForm onSubmit={handleAddCluster}>
          <CModalBody>
            <div className="mb-3">
              <CFormLabel htmlFor="clusterName">Cluster Name</CFormLabel>
              <CFormInput id="clusterName" placeholder="My Kafka Cluster" required />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="bootstrapServers">Bootstrap Servers</CFormLabel>
              <CFormInput id="bootstrapServers" placeholder="localhost:9092" required />
            </div>
            <div className="mb-3">
              <CFormLabel htmlFor="securityProtocol">Security Protocol</CFormLabel>
              <CFormSelect id="securityProtocol">
                <option value="PLAINTEXT">PLAINTEXT</option>
                <option value="SSL">SSL</option>
                <option value="SASL_PLAINTEXT">SASL_PLAINTEXT</option>
                <option value="SASL_SSL">SASL_SSL</option>
              </CFormSelect>
            </div>
          </CModalBody>
          <CModalFooter>
            <CButton color="secondary" onClick={() => setShowAddClusterModal(false)}>
              Cancel
            </CButton>
            <CButton color="primary" type="submit">
              Add Cluster
            </CButton>
          </CModalFooter>
        </CForm>
      </CModal>
    </>
  );
};

export default Kafka;
