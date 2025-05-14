import React from "react";
import { CCard, CCardBody, CCardHeader, CRow, CCol } from "@coreui/react";
import { useAuth } from "../hooks/useAuth";

const Profile = () => {
  const { user } = useAuth();
  
  return (
    <>
      <h2 className="mb-4">User Profile</h2>
      <CCard className="mb-4">
        <CCardHeader>Profile Information</CCardHeader>
        <CCardBody>
          <p><strong>Username:</strong> {user?.username || 'N/A'}</p>
          <p><strong>Role:</strong> {user?.role || 'N/A'}</p>
        </CCardBody>
      </CCard>
    </>
  );
};

export default Profile;
