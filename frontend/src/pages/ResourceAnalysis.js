import React, { useState, useEffect } from "react";
import { useParams, useNavigate, useSearchParams } from "react-router-dom";
import { Alert, Spinner, Row, Col, Card, CardBody, Button } from "reactstrap";
import { fetchWithAuth, analyzeAwsResource, askAwsResourceQuestion } from "../services/api";
import PageHeader from "../components/layout/PageHeader";
import BaseAnalysis from "../components/common/BaseAnalysis";

const ResourceAnalysis = () => {
  const { id } = useParams();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const initialQuestion = searchParams.get('question');
  
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [resource, setResource] = useState(null);
  const [analysisResult, setAnalysisResult] = useState(null);
  const [selectedWorkflow, setSelectedWorkflow] = useState(null);
  const [analysisHistory, setAnalysisHistory] = useState([]);
  const [workflows, setWorkflows] = useState([]);
  const [workflowsLoading, setWorkflowsLoading] = useState(false);

  // Fetch resource details
  useEffect(() => {
    const fetchResourceDetails = async () => {
      try {
        setLoading(true);
        const response = await fetchWithAuth(`/api/aws/resources/${id}`);
        if (response.ok) {
          const data = await response.json();
          setResource(data);
          
          // Also fetch the available workflows for this resource type
          await fetchWorkflows(data.resource_type);
        } else {
          throw new Error("Failed to fetch resource details");
        }
      } catch (err) {
        setError(err.message);
      } finally {
        setLoading(false);
      }
    };

    fetchResourceDetails();
  }, [id]);
  
  // Function to fetch available workflows for a resource type
  const fetchWorkflows = async (resourceType) => {
    try {
      console.log(`Fetching workflows for resource type: ${resourceType}`);
      setWorkflowsLoading(true);
      const backendUrl = process.env.REACT_APP_API_URL || "http://localhost:8080";
      console.log(`Using backend URL: ${backendUrl}`);
      
      // Normalize resource type to consistent format
      const normalizedResourceType = resourceType.trim();
      console.log(`Normalized resource type: ${normalizedResourceType}`);
      
      // Make the API call
      console.log(`Making request to: /api/aws/analytics/workflows/${normalizedResourceType}`);
      const response = await fetchWithAuth(`/api/aws/analytics/workflows/${normalizedResourceType}`);
      console.log(`Workflow fetch response status: ${response.status}`);
      
      if (response.ok) {
        const data = await response.json();
        console.log(`Workflows fetched successfully:`, data);
        // Ensure workflow IDs are present and valid
        const validWorkflows = (data.workflows || []).map(workflow => {
          // If the workflow doesn't have an ID, use the workflow_id field or generate a unique ID
          if (!workflow.id && workflow.workflow_id) {
            workflow.id = workflow.workflow_id;
          }
          return workflow;
        }).filter(workflow => workflow.id); // Only keep workflows with valid IDs
        
        console.log(`Valid workflows after processing:`, validWorkflows);
        setWorkflows(validWorkflows);
      } else {
        console.error(`Failed to fetch workflows - Status: ${response.status}`);
        // Try to get more information from the response
        let errorText = '';
        try {
          errorText = await response.text();
          console.error(`Response error text: ${errorText}`);
        } catch (e) {
          console.error(`Could not read response text: ${e.message}`);
        }
        throw new Error(`Failed to fetch workflows for ${resourceType}: ${errorText}`);
      }
    } catch (err) {
      console.error("Error fetching workflows:", err);
      // Create a user-friendly error message
      const friendlyError = `Unable to fetch analysis workflows for ${resourceType}. This may be due to an unsupported resource type or a server issue. Please try again or contact support if the issue persists.`;
      // Don't set error here as it's not critical and would override the main error state
      // Instead show a notification or warning
      setWorkflows([]);  // Reset workflows to empty array
    } finally {
      setWorkflowsLoading(false);
    }
  };
  
  // If there's an initial question from URL params, ask it once resource is loaded
  useEffect(() => {
    if (initialQuestion && resource && !loading) {
      askRelatedQuestion(initialQuestion);
    }
  }, [resource, initialQuestion]);

  const runAnalysis = async (workflowId) => {
    // If workflowId is null, undefined, or an empty string, show an error
    if (!workflowId) {
      console.error("Cannot run analysis: Workflow ID is missing or invalid");
      setError("Cannot run analysis: Please select a valid workflow");
      return;
    }
    
    // If selecting the same workflow that's already active, do nothing
    if (workflowId === selectedWorkflow) {
      console.log(`Workflow ${workflowId} is already selected`);
      return;
    }
    
    console.log(`Starting new analysis with workflow: ${workflowId}`);
    
    // Reset all state for a fresh analysis
    setAnalysisResult(null); // Clear previous results
    setAnalysisHistory([]); // Reset history
    setError(null); // Clear any errors
    
    // Set the new workflow and start loading
    setSelectedWorkflow(workflowId);
    setLoading(true);
    
    try {
      // Validate inputs before making the API call
      if (!resource || !resource.arn) {
        throw new Error("Resource information is missing or invalid");
      }
      
      console.log(`Running analysis with: Resource ARN=${resource.arn}, Workflow ID=${workflowId}`);
      
      // Call our backend API for resource analysis
      const result = await analyzeAwsResource(resource.arn, workflowId);
      console.log("Analysis result received:", result);
      setAnalysisResult(result);
    } catch (err) {
      console.error("Analysis failed:", err);
      setError("Failed to run analysis: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const askRelatedQuestion = async (question) => {
    try {
      setLoading(true);
      console.log(`Asking related question: ${question}`);
      
      // Call the backend API with the question
      const result = await askAwsResourceQuestion(resource.arn, question, selectedWorkflow);
      
      // Update UI with the result
      setAnalysisResult(result);
      
      // Record this question in the analysis history
      setAnalysisHistory(prevHistory => {
        const newHistory = [...prevHistory, { 
          question: question, 
          result: result,
          timestamp: new Date().toISOString()
        }];
        console.log("Updated analysis history:", newHistory);
        return newHistory;
      });
      
    } catch (err) {
      setError("Failed to process question: " + (err.response?.data?.message || err.message));
      console.error("Error processing question:", err);
    } finally {
      setLoading(false);
    }
  };

  const getResourceTypeIcon = (resourceType) => {
    const iconMap = {
      "EC2Instance": "server",
      "RdsInstance": "database",
      "DynamoDbTable": "table",
      "S3Bucket": "archive",
      "ElasticacheCluster": "memory",
      "SqsQueue": "exchange",
      "KinesisStream": "stream",
      "LambdaFunction": "code",
      "ElasticLoadBalancer": "sitemap",
      "ApiGatewayApi": "plug",
      "CloudFrontDistribution": "globe",
      "Route53HostedZone": "route",
      "ElasticsearchDomain": "search",
      "ElasticBeanstalkEnvironment": "leaf",
      "EcsCluster": "cubes"
    };
    
    return iconMap[resourceType] || "cloud";
  };

  if (loading && !resource && !analysisResult) {
    return (
      <div className="d-flex justify-content-center align-items-center" style={{ height: "300px" }}>
        <Spinner color="primary" />
      </div>
    );
  }

  if (error && !resource) {
    return (
      <div>
        <PageHeader 
          title="Resource Analysis"
          icon="fa-exclamation-triangle"
          breadcrumbs={[
            { name: "Cloud", path: "/cloud" },
            { name: "Resources", path: "/cloud" },
            { name: "Analysis", active: true }
          ]}
        />
        <Row>
          <Col>
            <Card>
              <CardBody>
                <Alert color="danger">
                  <h4 className="alert-heading">Error Loading Resource</h4>
                  <p>{error}</p>
                  <hr />
                  <p className="mb-0">
                    <Button color="secondary" onClick={() => navigate("/cloud")}>
                      Return to Resources
                    </Button>
                  </p>
                </Alert>
              </CardBody>
            </Card>
          </Col>
        </Row>
      </div>
    );
  }

  return (
    <div>
      <PageHeader 
        title={`${formatResourceType(resource?.resource_type) || 'Resource'} Analysis`}
        icon={`fa-${getResourceTypeIcon(resource?.resource_type)}`}
        breadcrumbs={[
          { name: "Cloud", path: "/cloud" },
          { name: "Resources", path: "/cloud" },
          { name: formatResourceType(resource?.resource_type) || "Resource", path: "/cloud" },
          { name: "Analysis", active: true }
        ]}
      />

      <Row>
        <Col lg={12}>
          <BaseAnalysis
            key={selectedWorkflow || 'initial'} // Force remount when workflow changes
            title={`${resource?.name || resource?.resource_id || 'Resource'} Analysis`}
            resource={resource}
            workflows={workflows}
            onRunAnalysis={runAnalysis}
            result={analysisResult}
            loading={loading || workflowsLoading}
            error={error}
            selectedWorkflow={selectedWorkflow}
            onAskQuestion={askRelatedQuestion}
            analysisHistory={analysisHistory}
          />
        </Col>
      </Row>
    </div>
  );
};

// Helper function to format resource types for display
const formatResourceType = (resourceType) => {
  if (!resourceType) return '';
  
  // Handle special cases
  switch (resourceType) {
    case 'EC2Instance':
      return 'EC2 Instance';
    case 'RdsInstance':
      return 'RDS Instance';
    case 'DynamoDbTable':
      return 'DynamoDB Table';
    case 'S3Bucket':
      return 'S3 Bucket';
    case 'ElasticacheCluster':
      return 'ElastiCache Cluster';
    case 'SqsQueue':
      return 'SQS Queue';
    case 'KinesisStream':
      return 'Kinesis Stream';
    case 'LambdaFunction':
      return 'Lambda Function';
    default:
      // Convert camelCase to words with spaces
      return resourceType
        .replace(/([A-Z])/g, ' $1')
        .replace(/^./, (str) => str.toUpperCase())
        .trim();
  }
};

export default ResourceAnalysis;
