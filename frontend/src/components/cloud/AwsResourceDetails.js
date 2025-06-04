import React, { useState, useEffect } from "react";
import {
  Modal,
  ModalHeader,
  ModalBody,
  Row,
  Col,
  Nav,
  NavItem,
  NavLink,
  TabContent,
  TabPane,
  Card,
  CardBody,
  Badge,
  Button,
  Table,
  FormGroup,
  Label,
  Input
} from "reactstrap";
import classnames from "classnames";
import Spinner from "../common/Spinner";
import api from "../../services/api";
import ReactJson from "react-json-view";

const AwsResourceDetails = ({ resource, isOpen, toggle }) => {
  const [activeTab, setActiveTab] = useState("overview");
  const [dataPlaneAction, setDataPlaneAction] = useState(null);
  const [actionPayload, setActionPayload] = useState("");
  const [actionResult, setActionResult] = useState(null);
  const [actionLoading, setActionLoading] = useState(false);
  
  // Reset data plane action when resource changes
  useEffect(() => {
    setDataPlaneAction(null);
    setActionPayload("");
    setActionResult(null);
  }, [resource]);

  // Function to get default payload template based on resource type and action
  const getDefaultPayload = () => {
    if (!resource || !dataPlaneAction) return "{}";
    
    switch (resource.resource_type) {
      case "S3Bucket":
        if (dataPlaneAction === "getObject") {
          return JSON.stringify({
            key: "example-key.json"
          }, null, 2);
        }
        if (dataPlaneAction === "putObject") {
          return JSON.stringify({
            key: "example-key.json",
            content_type: "application/json",
            body: "Hello from Mayyam!"
          }, null, 2);
        }
        break;
      case "DynamoDbTable":
        if (dataPlaneAction === "getItem") {
          return JSON.stringify({
            key: {
              "id": { "S": "example-id" }
            }
          }, null, 2);
        }
        if (dataPlaneAction === "putItem") {
          return JSON.stringify({
            item: {
              "id": { "S": "example-id" },
              "name": { "S": "Example Item" },
              "created_at": { "S": new Date().toISOString() }
            }
          }, null, 2);
        }
        if (dataPlaneAction === "query") {
          return JSON.stringify({
            key_condition_expression: "id = :id",
            expression_attribute_values: {
              ":id": { "S": "example-id" }
            }
          }, null, 2);
        }
        break;
      case "SqsQueue":
        if (dataPlaneAction === "sendMessage") {
          return JSON.stringify({
            queue_url: resource.resource_data.queue_url || "",
            message_body: "Hello from Mayyam!",
            delay_seconds: 0
          }, null, 2);
        }
        if (dataPlaneAction === "receiveMessages") {
          return JSON.stringify({
            queue_url: resource.resource_data.queue_url || "",
            max_messages: 10,
            wait_time_seconds: 0
          }, null, 2);
        }
        break;
      case "KinesisStream":
        if (dataPlaneAction === "putRecord") {
          return JSON.stringify({
            stream_name: resource.resource_id,
            data: "Hello from Mayyam!",
            partition_key: "example-partition"
          }, null, 2);
        }
        break;
      default:
        return "{}";
    }
    
    return "{}";
  };

  // Execute data plane action
  const executeAction = async () => {
    if (!resource || !dataPlaneAction) return;
    
    setActionLoading(true);
    setActionResult(null);
    
    try {
      let endpoint = "";
      let payload = {};
      
      try {
        payload = JSON.parse(actionPayload);
      } catch (e) {
        setActionResult({
          error: "Invalid JSON payload"
        });
        setActionLoading(false);
        return;
      }
      
      const profile = resource.profile || "default";
      const region = resource.region;
      
      switch (resource.resource_type) {
        case "S3Bucket":
          if (dataPlaneAction === "getObject") {
            endpoint = `/api/aws-data/profiles/${profile}/s3/${resource.resource_id}/${payload.key}`;
            const response = await api.get(endpoint);
            setActionResult(response.data);
          } else if (dataPlaneAction === "putObject") {
            endpoint = `/api/aws-data/profiles/${profile}/regions/${region}/s3`;
            payload.bucket = resource.resource_id;
            const response = await api.post(endpoint, payload);
            setActionResult(response.data);
          }
          break;
          
        case "DynamoDbTable":
          endpoint = `/api/aws-data/profiles/${profile}/regions/${region}/dynamodb/${resource.resource_id}`;
          if (dataPlaneAction === "getItem") {
            endpoint += "/item";
            const response = await api.get(endpoint, { params: payload });
            setActionResult(response.data);
          } else if (dataPlaneAction === "putItem") {
            endpoint += "/item";
            const response = await api.post(endpoint, payload);
            setActionResult(response.data);
          } else if (dataPlaneAction === "query") {
            endpoint += "/query";
            const response = await api.post(endpoint, payload);
            setActionResult(response.data);
          }
          break;
          
        case "SqsQueue":
          if (dataPlaneAction === "sendMessage") {
            endpoint = `/api/aws-data/profiles/${profile}/regions/${region}/sqs/send`;
            const response = await api.post(endpoint, payload);
            setActionResult(response.data);
          } else if (dataPlaneAction === "receiveMessages") {
            endpoint = `/api/aws-data/profiles/${profile}/regions/${region}/sqs/receive`;
            const response = await api.post(endpoint, payload);
            setActionResult(response.data);
          }
          break;
          
        case "KinesisStream":
          if (dataPlaneAction === "putRecord") {
            endpoint = `/api/aws-data/profiles/${profile}/regions/${region}/kinesis`;
            const response = await api.post(endpoint, payload);
            setActionResult(response.data);
          }
          break;
          
        default:
          setActionResult({
            error: "No data plane actions available for this resource type"
          });
      }
    } catch (error) {
      console.error("Error executing data plane action:", error);
      setActionResult({
        error: error.response?.data?.message || error.message || "An error occurred"
      });
    } finally {
      setActionLoading(false);
    }
  };

  const renderDataPlaneActions = () => {
    if (!resource) return null;
    
    const actions = [];
    
    switch (resource.resource_type) {
      case "S3Bucket":
        actions.push({ id: "getObject", label: "Get Object" });
        actions.push({ id: "putObject", label: "Put Object" });
        break;
      case "DynamoDbTable":
        actions.push({ id: "getItem", label: "Get Item" });
        actions.push({ id: "putItem", label: "Put Item" });
        actions.push({ id: "query", label: "Query" });
        break;
      case "SqsQueue":
        actions.push({ id: "sendMessage", label: "Send Message" });
        actions.push({ id: "receiveMessages", label: "Receive Messages" });
        break;
      case "KinesisStream":
        actions.push({ id: "putRecord", label: "Put Record" });
        break;
      default:
        return <p className="text-muted">No data plane actions available for this resource type.</p>;
    }
    
    return (
      <div>
        <FormGroup>
          <Label for="dataPlaneAction">Action</Label>
          <Input
            type="select"
            id="dataPlaneAction"
            value={dataPlaneAction || ""}
            onChange={(e) => {
              const action = e.target.value;
              setDataPlaneAction(action);
              setActionPayload(action ? getDefaultPayload() : "");
              setActionResult(null);
            }}
          >
            <option value="">-- Select an action --</option>
            {actions.map(action => (
              <option key={action.id} value={action.id}>{action.label}</option>
            ))}
          </Input>
        </FormGroup>
        
        {dataPlaneAction && (
          <>
            <FormGroup>
              <Label for="actionPayload">Payload</Label>
              <Input
                type="textarea"
                id="actionPayload"
                value={actionPayload}
                onChange={(e) => setActionPayload(e.target.value)}
                rows={10}
                style={{ fontFamily: "monospace" }}
              />
            </FormGroup>
            
            <Button 
              color="primary" 
              onClick={executeAction}
              disabled={actionLoading}
            >
              Execute Action
            </Button>
            
            {actionLoading && <Spinner />}
            
            {actionResult && (
              <div className="mt-3">
                <h5>Result</h5>
                <Card>
                  <CardBody style={{ maxHeight: "300px", overflow: "auto" }}>
                    <ReactJson 
                      src={actionResult} 
                      name={null} 
                      displayDataTypes={false}
                      collapsed={1}
                    />
                  </CardBody>
                </Card>
              </div>
            )}
          </>
        )}
      </div>
    );
  };

  if (!resource) return null;

  return (
    <Modal isOpen={isOpen} toggle={toggle} size="lg">
      <ModalHeader toggle={toggle}>
        Resource Details: {resource.name || resource.resource_id}
      </ModalHeader>
      <ModalBody>
        <Nav tabs>
              <NavItem>
                <NavLink
                  className={classnames({ active: activeTab === "overview" })}
                  onClick={() => setActiveTab("overview")}
                >
                  Overview
                </NavLink>
              </NavItem>
              <NavItem>
                <NavLink
                  className={classnames({ active: activeTab === "tags" })}
                  onClick={() => setActiveTab("tags")}
                >
                  Tags
                </NavLink>
              </NavItem>
              <NavItem>
                <NavLink
                  className={classnames({ active: activeTab === "data" })}
                  onClick={() => setActiveTab("data")}
                >
                  Resource Data
                </NavLink>
              </NavItem>
              <NavItem>
                <NavLink
                  className={classnames({ active: activeTab === "actions" })}
                  onClick={() => setActiveTab("actions")}
                >
                  Data Plane Actions
                </NavLink>
              </NavItem>

            </Nav>
            
            <TabContent activeTab={activeTab}>
              <TabPane tabId="overview">
                <div className="p-3">
                  <Row>
                    <Col sm="3" className="font-weight-bold">Resource Type</Col>
                    <Col sm="9">
                      <Badge color="primary">{resource.resource_type}</Badge>
                    </Col>
                  </Row>
                  <hr />
                  <Row>
                    <Col sm="3" className="font-weight-bold">Resource ID</Col>
                    <Col sm="9">{resource.resource_id}</Col>
                  </Row>
                  <hr />
                  <Row>
                    <Col sm="3" className="font-weight-bold">Name</Col>
                    <Col sm="9">{resource.name || <em>No name</em>}</Col>
                  </Row>
                  <hr />
                  <Row>
                    <Col sm="3" className="font-weight-bold">ARN</Col>
                    <Col sm="9" style={{ wordBreak: "break-all" }}>{resource.arn}</Col>
                  </Row>
                  <hr />
                  <Row>
                    <Col sm="3" className="font-weight-bold">Account</Col>
                    <Col sm="9">{resource.account_id}</Col>
                  </Row>
                  <hr />
                  <Row>
                    <Col sm="3" className="font-weight-bold">Region</Col>
                    <Col sm="9">{resource.region}</Col>
                  </Row>
                  <hr />
                  <Row>
                    <Col sm="3" className="font-weight-bold">Last Updated</Col>
                    <Col sm="9">{new Date(resource.updated_at).toLocaleString()}</Col>
                  </Row>
                  <hr />
                  <Row>
                    <Col sm="3" className="font-weight-bold">Last Refreshed</Col>
                    <Col sm="9">{new Date(resource.last_refreshed).toLocaleString()}</Col>
                  </Row>
                </div>
              </TabPane>
              
              <TabPane tabId="tags">
                <div className="p-3">
                  {Object.keys(resource.tags).length === 0 ? (
                    <p className="text-muted">This resource has no tags.</p>
                  ) : (
                    <Table striped bordered>
                      <thead>
                        <tr>
                          <th>Key</th>
                          <th>Value</th>
                        </tr>
                      </thead>
                      <tbody>
                        {Object.entries(resource.tags).map(([key, value]) => (
                          <tr key={key}>
                            <td>{key}</td>
                            <td>{value}</td>
                          </tr>
                        ))}
                      </tbody>
                    </Table>
                  )}
                </div>
              </TabPane>
              
              <TabPane tabId="data">
                <div className="p-3">
                  <ReactJson 
                    src={resource.resource_data} 
                    name={null}
                    displayDataTypes={false}
                    collapsed={1}
                  />
                </div>
              </TabPane>
              
              <TabPane tabId="actions">
                <div className="p-3">
                  {renderDataPlaneActions()}
                </div>
              </TabPane>

            </TabContent>
      </ModalBody>
    </Modal>
  );
};

export default AwsResourceDetails;