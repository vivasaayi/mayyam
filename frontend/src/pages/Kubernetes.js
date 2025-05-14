import React from "react";
import { CCard, CCardBody, CCardHeader, CRow, CCol } from "@coreui/react";

const Kubernetes = () => {
  return (
    <>
      <h2 className="mb-4">Kubernetes Management</h2>
      <CCard className="mb-4">
        <CCardHeader>Connected Kubernetes Clusters</CCardHeader>
        <CCardBody>
          <p>No Kubernetes clusters connected yet. Use the button below to add your first Kubernetes cluster.</p>
        </CCardBody>
      </CCard>
    </>
  );
};

export default Kubernetes;
