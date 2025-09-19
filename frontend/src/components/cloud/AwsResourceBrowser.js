import React, { useState, useEffect, useCallback, useMemo } from "react";
import { useNavigate } from "react-router-dom";
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
import api, { getAwsAccounts } from "../../services/api";
import AwsResourceDetails from "./AwsResourceDetails";

const AwsResourceBrowser = () => {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [resources, setResources] = useState([]);
  const [totalResources, setTotalResources] = useState(0);
  const [error, setError] = useState(null);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(10);
  const [filter, setFilter] = useState({
    account_id: "",
    profile: "",
    region: "",
    resource_type: "",
    sync_id: "",
    tag_key: "",
    tag_value: "",
  });
  const [accounts, setAccounts] = useState([]);
  const [accountsLoading, setAccountsLoading] = useState(false);

  const [gridApi, setGridApi] = useState(null);
  const [gridColumnApi, setGridColumnApi] = useState(null);
  const [selectedResource, setSelectedResource] = useState(null);
  const [detailsModalOpen, setDetailsModalOpen] = useState(false);

  // Helper function to format resource types for display
  const formatResourceType = (resourceType) => {
    if (!resourceType) return '';

    // Handle special cases
    switch (resourceType) {
      case 'EC2Instance':
        return 'EC2';
      case 'RdsInstance':
        return 'RDS';
      case 'DynamoDbTable':
        return 'DynamoDB';
      case 'S3Bucket':
        return 'S3';
      case 'ElasticacheCluster':
        return 'ElastiCache';
      case 'SqsQueue':
        return 'SQS';
      case 'KinesisStream':
        return 'Kinesis';
      case 'LambdaFunction':
        return 'Lambda';
      default:
        // For grid, return shorter names
        return resourceType.replace(/([A-Z])/g, ' $1').trim();
    }
  };

  // Define AG Grid column definitions
  const columnDefs = useMemo(() => [
    {
      headerName: "Type",
      field: "resource_type",
      filter: true,
      sortable: true,
      width: 150,
      cellRenderer: (params) => {
        const resourceType = params.value;
        let badgeColor = "primary";
        let icon = "cloud";

        switch(resourceType) {
          case "EC2Instance":
            badgeColor = "info";
            icon = "server";
            break;
          case "S3Bucket":
            badgeColor = "success";
            icon = "archive";
            break;
          case "RdsInstance":
            badgeColor = "warning";
            icon = "database";
            break;
          case "DynamoDbTable":
            badgeColor = "danger";
            icon = "table";
            break;
          case "KinesisStream":
            badgeColor = "dark";
            icon = "stream";
            break;
          case "SqsQueue":
            badgeColor = "secondary";
            icon = "exchange";
            break;
          case "ElasticacheCluster":
            badgeColor = "info";
            icon = "memory";
            break;
          case "LambdaFunction":
            badgeColor = "primary";
            icon = "code";
            break;
          default:
            badgeColor = "primary";
            icon = "cloud";
        }

        return (
          <Badge color={badgeColor}>
            <i className={`fa fa-${icon} me-1`}></i>
            {formatResourceType(resourceType)}
          </Badge>
        );
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
      headerName: "Actions",
      field: "id",
      sortable: false,
      filter: false,
      width: 200,
      cellRenderer: (params) => {
        const resource = params.data;
        return (
          <div className="d-flex gap-2">
            <Button 
              color="primary" 
              size="sm" 
              onClick={() => viewResourceDetails(resource)}
            >
              <i className="fa fa-eye me-1"></i>View
            </Button>
            <Button
              color="success"
              size="sm"
              onClick={() => navigate(`/resource-analysis/${resource.id}`)}
            >
              <i className="fa fa-chart-line me-1"></i>Analyze
            </Button>
          </div>
        );
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
  ], []);

  const defaultColDef = useMemo(() => ({
    resizable: true,
  }), []);

  // Grid ready event handler
  const onGridReady = useCallback((params) => {
    setGridApi(params.api);
    setGridColumnApi(params.columnApi);
  }, []);

  // Load AWS accounts on component mount
  useEffect(() => {
    const fetchAccounts = async () => {
      try {
        setAccountsLoading(true);
        const data = await getAwsAccounts();
        setAccounts(data);
      } catch (error) {
        console.error("Error fetching AWS accounts:", error);
      } finally {
        setAccountsLoading(false);
      }
    };

    fetchAccounts();
  }, []);

  // Load resources on initial render or when filter/pagination changes
  useEffect(() => {
    fetchResources();
  }, [page, pageSize]);

  // Initialize sync_id from URL if present
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const s = params.get('sync_id');
    if (s) {
      setFilter((prev) => ({ ...prev, sync_id: s }));
    }
  }, []);

  const fetchResources = async () => {
    setLoading(true);
    setError(null);
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
      setError(error.response?.data?.message || error.message || "Failed to fetch AWS resources");
      setResources([]);
      setTotalResources(0);
      if (gridApi) {
        gridApi.setRowData([]);
      }
    } finally {
      setLoading(false);
    }
  };

  const handleFilterChange = (name, value) => {
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
    setError(null);
    try {
      if (filter.account_id) {
        // Sync a specific account
        await api.post(`/api/aws/accounts/${filter.account_id}/sync`);
      } else {
        // Sync all accounts or use legacy sync
        await api.post('/api/aws/accounts/sync');
      }

      // After sync, refresh the resources
      fetchResources();
    } catch (error) {
      console.error("Error syncing AWS resources:", error);
      setError(error.response?.data?.message || error.message || "Failed to sync AWS resources");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="animated fadeIn">
      <Card>
        <CardHeader>
          <div className="d-flex justify-content-between align-items-center">
            <div>
              <i className="fa fa-cloud"></i> AWS Resources
            </div>
            <div>
              <Button color="primary" onClick={syncResources} disabled={loading || accountsLoading} className="d-flex align-items-center">
                {loading ? (
                  <>
                    <Spinner size="sm" className="me-1" />
                    <span>Syncing...</span>
                  </>
                ) : (
                  <>
                    <i className="fa fa-sync me-1"></i> 
                    <span>Sync Resources</span>
                  </>
                )}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardBody>
          {error && (
            <div className="alert alert-danger" role="alert">
              <strong>Error:</strong> {error}
            </div>
          )}
          <Form onSubmit={applyFilters} className="mb-4">
            <Row className="g-3">
              <Col lg={2} md={4} sm={6}>
                <FormGroup>
                  <Label for="account_id">Account ID</Label>
                  <Input
                    type="select"
                    name="account_id"
                    id="account_id"
                    value={filter.account_id}
                    onChange={(e) => handleFilterChange(e.target.name, e.target.value)}
                    disabled={accountsLoading}
                  >
                    <option value="">Select Account</option>
                    {accounts.map((account) => (
                      <option key={account.id} value={account.account_id}>
                        {account.account_name} ({account.account_id})
                      </option>
                    ))}
                  </Input>
                </FormGroup>
              </Col>
              <Col lg={2} md={4} sm={6}>
                <FormGroup>
                  <Label for="sync_id">Sync ID</Label>
                  <Input
                    type="text"
                    name="sync_id"
                    id="sync_id"
                    value={filter.sync_id}
                    onChange={(e) => handleFilterChange(e.target.name, e.target.value)}
                    placeholder="Filter by sync run"
                  />
                </FormGroup>
              </Col>
              <Col lg={2} md={4} sm={6}>
                <FormGroup>
                  <Label for="profile">Profile</Label>
                  <Input
                    type="text"
                    name="profile"
                    id="profile"
                    value={filter.profile}
                    onChange={(e) => handleFilterChange(e.target.name, e.target.value)}
                    placeholder="Profile"
                  />
                </FormGroup>
              </Col>
              <Col lg={2} md={4} sm={6}>
                <FormGroup>
                  <Label for="region">Region</Label>
                  <Input
                    type="text"
                    name="region"
                    id="region"
                    value={filter.region}
                    onChange={(e) => handleFilterChange(e.target.name, e.target.value)}
                    placeholder="Region"
                  />
                </FormGroup>
              </Col>
              <Col lg={3} md={6} sm={6}>
                <FormGroup>
                  <Label for="resource_type">Resource Type</Label>
                  <Input
                    type="select"
                    name="resource_type"
                    id="resource_type"
                    value={filter.resource_type}
                    onChange={(e) => handleFilterChange(e.target.name, e.target.value)}
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
                  <Button type="submit" color="primary" className="me-2" disabled={loading}>
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
