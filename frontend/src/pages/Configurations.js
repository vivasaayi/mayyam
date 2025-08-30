import React, { useState } from "react";
import {
  CContainer,
  CCard,
  CCardBody,
  CCardHeader,
  CAlert
} from "@coreui/react";
import SharedResourcesNav from "../components/common/SharedResourcesNav";

const Configurations = () => {
  const [activeResource, setActiveResource] = useState("configurations");

  return (
    <CContainer fluid>
      <SharedResourcesNav 
        activeResource={activeResource}
        onResourceChange={setActiveResource}
      >
        <CCard>
          <CCardHeader>
            <h5>Shared Configurations</h5>
            <p className="text-medium-emphasis small">
              Manage configuration settings that apply across all systems.
            </p>
          </CCardHeader>
          <CCardBody>
            <CAlert color="info">
              <h4>Coming Soon</h4>
              <p>Shared configurations management is under development and will be available soon.</p>
              <p>This feature will allow you to create and manage system-wide configuration settings.</p>
            </CAlert>
          </CCardBody>
        </CCard>
      </SharedResourcesNav>
    </CContainer>
  );
};

export default Configurations;
