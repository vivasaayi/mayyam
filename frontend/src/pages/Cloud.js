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
  Row, Col, Nav, NavItem, NavLink, TabContent, TabPane, Card, CardBody, Button
} from "reactstrap";
import { useNavigate } from "react-router-dom";
import classnames from "classnames";
import PageHeader from "../components/layout/PageHeader";
import AwsResourceBrowser from "../components/cloud/AwsResourceBrowser";
import AwsAccountManagement from "../components/cloud/AwsAccountManagement";

const Cloud = () => {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState("aws");

  const toggleTab = (tab) => {
    if (activeTab !== tab) {
      setActiveTab(tab);
    }
  };

  return (
    <div>
      <PageHeader title="Cloud Accounts" icon="fa-cloud" />
      
      <Nav tabs>
        <NavItem>
          <NavLink
            className={classnames({ active: activeTab === "aws" })}
            onClick={() => toggleTab("aws")}
          >
            <i className="fab fa-aws mr-2"></i>
            AWS
          </NavLink>
        </NavItem>
        <NavItem>
          <NavLink
            className={classnames({ active: activeTab === "azure" })}
            onClick={() => toggleTab("azure")}
          >
            <i className="fab fa-microsoft mr-2"></i>
            Azure
          </NavLink>
        </NavItem>
        <NavItem>
          <NavLink
            className={classnames({ active: activeTab === "gcp" })}
            onClick={() => toggleTab("gcp")}
          >
            <i className="fab fa-google mr-2"></i>
            GCP
          </NavLink>
        </NavItem>
      </Nav>
      
      <TabContent activeTab={activeTab}>
        <TabPane tabId="aws">
          <Row className="mb-4">
            <Col>
              <AwsAccountManagement />
            </Col>
          </Row>
        </TabPane>
        
        <TabPane tabId="azure">
          <Row className="mt-3">
            <Col>
              <h3>Azure Resources</h3>
              <p>Azure cloud resources integration is coming soon.</p>
            </Col>
          </Row>
        </TabPane>
        
        <TabPane tabId="gcp">
          <Row className="mt-3">
            <Col>
              <h3>Google Cloud Resources</h3>
              <p>Google Cloud Platform resources integration is coming soon.</p>
            </Col>
          </Row>
        </TabPane>
      </TabContent>
    </div>
  );
};

export default Cloud;
