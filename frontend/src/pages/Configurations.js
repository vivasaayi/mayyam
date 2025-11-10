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
