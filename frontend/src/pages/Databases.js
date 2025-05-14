import React from "react";
import { CCard, CCardBody, CCardHeader, CRow, CCol } from "@coreui/react";

const Databases = () => {
  return (
    <>
      <h2 className="mb-4">Database Management</h2>
      <CCard className="mb-4">
        <CCardHeader>Connected Databases</CCardHeader>
        <CCardBody>
          <p>No databases connected yet. Use the button below to add your first database connection.</p>
        </CCardBody>
      </CCard>
    </>
  );
};

export default Databases;
