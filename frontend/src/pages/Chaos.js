import React from "react";
import { CCard, CCardBody, CCardHeader, CRow, CCol } from "@coreui/react";

const Chaos = () => {
  return (
    <>
      <h2 className="mb-4">Chaos Engineering</h2>
      <CCard className="mb-4">
        <CCardHeader>Chaos Experiments</CCardHeader>
        <CCardBody>
          <p>No chaos experiments configured yet. Use the button below to set up your first experiment.</p>
        </CCardBody>
      </CCard>
    </>
  );
};

export default Chaos;
