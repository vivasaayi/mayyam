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


import React, { useState, useEffect } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CContainer,
  CRow,
  CCol,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane,
  CSpinner,
  CAlert
} from "@coreui/react";
import QueryTemplateManager from "../components/database/QueryTemplateManager";
import QueryTemplateService from "../services/queryTemplateService";
import SharedResourcesNav from "../components/common/SharedResourcesNav";
import "../styles/QueryTemplates.css"; // Import the CSS file

const QueryTemplates = () => {
  const [activeTab, setActiveTab] = useState("common");
  const [activeResource, setActiveResource] = useState("queryTemplates");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [commonTemplates, setCommonTemplates] = useState([]);
  const [mysqlTemplates, setMysqlTemplates] = useState([]);
  const [postgresTemplates, setPostgresTemplates] = useState([]);

  // Load templates for different types
  useEffect(() => {
    const loadAllTemplates = async () => {
      setLoading(true);
      setError(null);
      try {
        // Load common templates
        const common = await QueryTemplateService.getCommonTemplates();
        setCommonTemplates(common);
        
        // Load MySQL templates
        const mysql = await QueryTemplateService.getTemplatesByType("mysql");
        setMysqlTemplates(mysql);
        
        // Load PostgreSQL templates
        const postgres = await QueryTemplateService.getTemplatesByType("postgresql");
        setPostgresTemplates(postgres);
        
      } catch (err) {
        console.error("Failed to load templates:", err);
        setError("Failed to load templates. Please try again later.");
      } finally {
        setLoading(false);
      }
    };
    
    loadAllTemplates();
  }, []);

  // Mock connection objects for the template managers
  const mockConnections = {
    mysql: { connection_type: "mysql", name: "MySQL Templates" },
    postgresql: { connection_type: "postgresql", name: "PostgreSQL Templates" },
    common: { connection_type: "", name: "Common Templates" }
  };

  const handleTemplateSelect = (template) => {
    // This function is not used in the standalone version but is required by the component
    console.log("Template selected:", template);
  };

  return (
    <CContainer fluid>
      <SharedResourcesNav 
        activeResource={activeResource}
        onResourceChange={setActiveResource}
      >
        <CCard>
          <CCardHeader>
            <h5>Query Templates</h5>
            <p className="text-medium-emphasis small">
              Create and manage SQL query templates for your database connections. Templates can be database-specific or common across all database types.
            </p>
          </CCardHeader>
          <CCardBody>
            {loading ? (
              <div className="text-center p-3">
                <CSpinner />
                <p className="mt-2">Loading templates...</p>
              </div>
            ) : error ? (
              <CAlert color="danger">{error}</CAlert>
            ) : (
              <>
                <CNav variant="tabs" role="tablist" className="mb-3">
                  <CNavItem>
                    <CNavLink 
                      active={activeTab === "common"}
                      onClick={() => setActiveTab("common")}
                      role="tab"
                    >
                      Common Templates
                    </CNavLink>
                  </CNavItem>
                  <CNavItem>
                    <CNavLink 
                      active={activeTab === "mysql"}
                      onClick={() => setActiveTab("mysql")}
                      role="tab"
                    >
                      MySQL Templates
                    </CNavLink>
                  </CNavItem>
                  <CNavItem>
                    <CNavLink 
                      active={activeTab === "postgresql"}
                      onClick={() => setActiveTab("postgresql")}
                      role="tab"
                    >
                      PostgreSQL Templates
                    </CNavLink>
                  </CNavItem>
                </CNav>
                
                <CTabContent>
                  <CTabPane visible={activeTab === "common"} role="tabpanel">
                    <QueryTemplateManager 
                      connection={mockConnections.common} 
                      onTemplateSelect={handleTemplateSelect}
                      initialTemplates={commonTemplates}
                    />
                  </CTabPane>
                  <CTabPane visible={activeTab === "mysql"} role="tabpanel">
                    <QueryTemplateManager 
                      connection={mockConnections.mysql} 
                      onTemplateSelect={handleTemplateSelect}
                      initialTemplates={mysqlTemplates}
                    />
                  </CTabPane>
                  <CTabPane visible={activeTab === "postgresql"} role="tabpanel">
                    <QueryTemplateManager 
                      connection={mockConnections.postgresql} 
                      onTemplateSelect={handleTemplateSelect}
                      initialTemplates={postgresTemplates}
                    />
                  </CTabPane>
                </CTabContent>
              </>
            )}
          </CCardBody>
        </CCard>
      </SharedResourcesNav>
    </CContainer>
  );
};

export default QueryTemplates;
