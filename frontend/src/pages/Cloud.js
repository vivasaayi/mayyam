import React from "react";
import { CCard, CCardBody, CCardHeader, CRow, CCol } from "@coreui/react";

const Cloud = () => {
  return (
    <>
      <h2 className="mb-4">Cloud Management</h2>
      <CCard className="mb-4">
        <CCardHeader>Connected Cloud Accounts</CCardHeader>
        <CCardBody>
          <p>No cloud accounts connected yet. Use the button below to add your first cloud account.</p>
        </CCardBody>
      </CCard>
    </>
  );
};

export default Cloud;
