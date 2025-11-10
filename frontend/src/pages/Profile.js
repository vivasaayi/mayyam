// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


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
