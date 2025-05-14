import React from "react";
import { CCard, CCardBody, CContainer, CButton } from "@coreui/react";
import { Link } from "react-router-dom";

const NotFound = () => {
  return (
    <CContainer className="d-flex align-items-center justify-content-center" style={{ minHeight: "70vh" }}>
      <CCard className="text-center" style={{ maxWidth: "500px" }}>
        <CCardBody>
          <h1 className="display-1">404</h1>
          <h2 className="mb-4">Page Not Found</h2>
          <p className="mb-4">
            The page you are looking for might have been removed, had its name changed,
            or is temporarily unavailable.
          </p>
          <Link to="/">
            <CButton color="primary">Go to Dashboard</CButton>
          </Link>
        </CCardBody>
      </CCard>
    </CContainer>
  );
};

export default NotFound;
