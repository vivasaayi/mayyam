import React, { useState, useEffect, useCallback, useMemo } from "react";
import { AgGridReact } from "ag-grid-react";
import "ag-grid-community/styles/ag-grid.css";
import "ag-grid-community/styles/ag-theme-alpine.css";
import {
  Card,
  CardHeader,
  CardBody,
  Button,
  Form,
  FormGroup,
  Label,
  Input,
  Row,
  Col,
  Badge,
} from "reactstrap";
import Spinner from "../common/Spinner";
import api from "../../services/api";
import AwsResourceDetails from "./AwsResourceDetails";

const AwsResourceBrowser = () => {
  const [loading, setLoading] = useState(false);
  const [resources, setResources] = useState([]);
  const [totalResources, setTotalResources] = useState(0);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(10);
  const [filter, setFilter] = useState({
    account_id: "",
    profile: "",
    region: "",
    resource_type: "",
    tag_key: "",
    tag_value: "",
  });
  
  const [gridApi, setGridApi] = useState(null);
  const [gridColumnApi, setGridColumnApi] = useState(null);
  const [selectedResource, setSelectedResource] = useState(null);
  const [detailsModalOpen, setDetailsModalOpen] = useState(false);
  
  // Define AG Grid column definitions
  const columnDefs = useMemo(() => [
    {
      headerName: "Type",
      field: "resource_type",
      filter: true,
      sortable: true,
      width: 130,
      cellRenderer: (params) => {
        const resourceType = params.value;
        let badgeColor = "primary";
        
        switch(resourceType) {
          case "EC2Instance":
            badgeColor = "info";
            break;
          case "S3Bucket":
            badgeColor = "success";
            break;
          case "RdsInstance":
            badgeColor = "warning";
            break;
          case "DynamoDbTable":
            badgeColor = "danger";
            break;
          case "KinesisStream":
            badgeColor = "dark";
            break;
          case "SqsQueue":
            badgeColor = "secondary";
            break;
          default:
            badgeColor = "primary";
        }
        
        return <Badge color={badgeColor}>{resourceType}</Badge>;
      }
    },
    {
      headerName: "Name/ID",
      field: "name",
      filter: true,
      sortable: true,
      width: 200,
      cellRenderer: (params) => {
        const name = params.data.name || params.data.resource_id;
        return name;
      }
    },
    {
      headerName: "Account",
      field: "account_id",
      filter: true,
      sortable: true,
      width: 120,
    },
    {
      headerName: "Region",
      field: "region",
      filter: true,
      sortable: true,
      width: 120,
    },
    {
      headerName: "ARN",
      field: "arn",
      filter: true,
      sortable: true,
      width: 350,
    },
    {
      headerName: "Tags",
      field: "tags",
      width: 220,
      cellRenderer: (params) => {
        const tags = params.value;
        if (!tags || Object.keys(tags).length === 0) {
          return <span className="text-muted">No tags</span>;
        }
        
        return (
          <div style={{ maxHeight: "60px", overflow: "auto" }}>
            {Object.entries(tags).map(([key, value]) => (
              <Badge 
                key={key} 
                color="light" 
                className="mr-1 mb-1" 
                style={{ margin: "2px", display: "inline-block" }}
              >
                {key}: {value}
              </Badge>
            ))}
          </div>
        );
      }
    },
    {
      headerName: "Last Updated",
      field: "updated_at",
      filter: true,
      sortable: true,
      width: 180,
      cellRenderer: (params) => {
        const date = new Date(params.value);
        return date.toLocaleString();
      }
    },
    {
      headerName: "Actions",
      field: "id",
      sortable: false,
      filter: false,
      width: 120,
      cellRenderer: (params) => {
        return (
          <div>
            <Button 
              color="primary" 
              size="sm" 
              onClick={() => viewResourceDetails(params.data)}
              style={{ marginRight: "5px" }}
            >
              View
            </Button>
          </div>
        );
      }
    },
  ], []);
  
  const defaultColDef = useMemo(() => ({
    resizable: true,
  }), []);
  
  // Grid ready event handler
  const onGridReady = useCallback((params) => {
    setGridApi(params.api);
    setGridColumnApi(params.columnApi);
  }, []);
  
  // Load resources on initial render or when filter/pagination changes
  useEffect(() => {
    fetchResources();
  }, [page, pageSize]);
  
  const fetchResources = async () => {
    setLoading(true);
    try {
      const queryParams = new URLSearchParams();
      
      // Add filter parameters
      Object.entries(filter).forEach(([key, value]) => {
        if (value) {
          queryParams.append(key, value);
        }
      });
      
      // Add pagination parameters
      queryParams.append('page', page);
      queryParams.append('page_size', pageSize);
      
      const response = await api.get(`/api/aws/resources?${queryParams.toString()}`);
      setResources(response.data.resources);
      setTotalResources(response.data.total);
      
      // If grid is ready, update row data
      if (gridApi) {
        gridApi.setRowData(response.data.resources);
      }
    } catch (error) {
      console.error("Error fetching AWS resources:", error);
      // Handle error appropriately
    } finally {
      setLoading(false);
    }
  };
  
  const handleFilterChange = (e) => {
    const { name, value } = e.target;
    setFilter(prevFilter => ({
      ...prevFilter,
      [name]: value
    }));
  };
  
  const applyFilters = (e) => {
    e.preventDefault();
    // Reset pagination when applying new filters
    setPage(0);
    fetchResources();
  };
  
  const viewResourceDetails = (resource) => {
    setSelectedResource(resource);
    setDetailsModalOpen(true);
  };
  
  const syncResources = async () => {
    setLoading(true);
    try {
      const syncRequest = {
        account_id: filter.account_id || "default",
        profile: filter.profile || null,
        region: filter.region || "us-east-1",
        resource_types: filter.resource_type ? [filter.resource_type] : null
      };
      
      await api.post('/api/aws/sync', syncRequest);
      // After sync, refresh the resources
      fetchResources();
    } catch (error) {
      console.error("Error syncing AWS resources:", error);
      // Handle error appropriately
    } finally {
      setLoading(false);
    }
  };
  
  return (
    <div className="animated fadeIn">
      <Card>
        <CardHeader>
          <i className="fa fa-cloud"></i> AWS Resources
          <div className="card-header-actions">
            <Button color="primary" onClick={syncResources} disabled={loading}>
              <i className="fa fa-sync"></i> Sync Resources
            </Button>
          </div>
        </CardHeader>
        <CardBody>
          <Form onSubmit={applyFilters}>
            <Row>
              <Col md={2}>
                <FormGroup>
                  <Label for="account_id">Account ID</Label>
                  <Input
                    type="text"
                    name="account_id"
                    id="account_id"
                    value={filter.account_id}
                    onChange={handleFilterChange}
                    placeholder="Account ID"
                  />
                </FormGroup>
              </Col>
              <Col md={2}>
                <FormGroup>
                  <Label for="profile">Profile</Label>
                  <Input
                    type="text"
                    name="profile"
                    id="profile"
                    value={filter.profile}
                    onChange={handleFilterChange}
                    placeholder="Profile"
                  />
                </FormGroup>
              </Col>
              <Col md={2}>
                <FormGroup>
                  <Label for="region">Region</Label>
                  <Input
                    type="text"
                    name="region"
                    id="region"
                    value={filter.region}
                    onChange={handleFilterChange}
                    placeholder="Region"
                  />
                </FormGroup>
              </Col>
              <Col md={3}>
                <FormGroup>
                  <Label for="resource_type">Resource Type</Label>
                  <Input
                    type="select"
                    name="resource_type"
                    id="resource_type"
                    value={filter.resource_type}
                    onChange={handleFilterChange}
                  >
                    <option value="">All Types</option>
                    <option value="EC2Instance">EC2 Instances</option>
                    <option value="S3Bucket">S3 Buckets</option>
                    <option value="RdsInstance">RDS Instances</option>
                    <option value="DynamoDbTable">DynamoDB Tables</option>
                    <option value="KinesisStream">Kinesis Streams</option>
                    <option value="SqsQueue">SQS Queues</option>
                    <option value="SnsTopic">SNS Topics</option>
                    <option value="LambdaFunction">Lambda Functions</option>
                  </Input>
                </FormGroup>
              </Col>
              <Col md={3} className="d-flex align-items-end">
                <FormGroup className="mb-0">
                  <Button type="submit" color="primary" className="mr-2" disabled={loading}>
                    Apply Filters
                  </Button>
                  <Button 
                    type="button" 
                    color="secondary" 
                    onClick={() => {
                      setFilter({
                        account_id: "",
                        profile: "",
                        region: "",
                        resource_type: "",
                        tag_key: "",
                        tag_value: "",
                      });
                    }}
                    disabled={loading}
                  >
                    Clear
                  </Button>
                </FormGroup>
              </Col>
            </Row>
          </Form>
          
          {loading && <Spinner />}
          
          <div className="mt-4">
            <p>Total Resources: {totalResources}</p>
          </div>
          
          <div 
            className="ag-theme-alpine" 
            style={{ height: '600px', width: '100%' }}
          >
            <AgGridReact
              columnDefs={columnDefs}
              rowData={resources}
              defaultColDef={defaultColDef}
              pagination={true}
              paginationPageSize={pageSize}
              onGridReady={onGridReady}
              animateRows={true}
              enableCellTextSelection={true}
              rowSelection="multiple"
            />
          </div>
        </CardBody>
      </Card>
      
      {/* Add the Resource Details Modal */}
      <AwsResourceDetails 
        resource={selectedResource}
        isOpen={detailsModalOpen}
        toggle={() => setDetailsModalOpen(!detailsModalOpen)}
      />
    </div>
  );
};

export default AwsResourceBrowser;