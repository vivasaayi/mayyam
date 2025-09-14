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
      <PageHeader title="Cloud Resources" icon="fa-cloud" />
      
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
          <Row className="mb-3">
            <Col>
              <Card>
                <CardBody>
                  <h5>Quick Actions</h5>
                  <p className="text-muted">Specialized analysis tools for AWS services</p>
                  <Button 
                    color="primary" 
                    className="me-2"
                    onClick={() => navigate('/kinesis-analysis')}
                  >
                    <i className="fas fa-stream me-2"></i>
                    Kinesis Stream Analysis
                  </Button>
                  <Button color="outline-secondary" disabled>
                    <i className="fas fa-database me-2"></i>
                    RDS Analysis (Coming Soon)
                  </Button>
                </CardBody>
              </Card>
            </Col>
          </Row>
          <Row>
            <Col>
              <AwsResourceBrowser />
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
