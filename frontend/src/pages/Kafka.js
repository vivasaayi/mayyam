import React from "react";
import { CCard, CCardBody, CCardHeader, CRow, CCol } from "@coreui/react";

const Kafka = () => {
  return (
    <>
      <h2 className="mb-4">Kafka Management</h2>
      <CCard className="mb-4">
        <CCardHeader>Connected Kafka Clusters</CCardHeader>
        <CCardBody>
          <p>No Kafka clusters connected yet. Use the button below to add your first Kafka connection.</p>
        </CCardBody>
      </CCard>
    </>
  );
};

export default Kafka;
