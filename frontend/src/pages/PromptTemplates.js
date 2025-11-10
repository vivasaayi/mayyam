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

const PromptTemplates = () => {
  const [activeResource, setActiveResource] = useState("promptTemplates");

  return (
    <CContainer fluid>
      <SharedResourcesNav 
        activeResource={activeResource}
        onResourceChange={setActiveResource}
      >
        <CCard>
          <CCardHeader>
            <h5>Prompt Templates</h5>
            <p className="text-medium-emphasis small">
              Create and manage reusable prompt templates for LLM interactions.
            </p>
          </CCardHeader>
          <CCardBody>
            <CAlert color="info">
              <h4>Coming Soon</h4>
              <p>Prompt template management is under development and will be available soon.</p>
              <p>This feature will allow you to create, edit, and manage reusable prompts for your AI-powered features.</p>
            </CAlert>
          </CCardBody>
        </CCard>
      </SharedResourcesNav>
    </CContainer>
  );
};

export default PromptTemplates;
