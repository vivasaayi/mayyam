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
