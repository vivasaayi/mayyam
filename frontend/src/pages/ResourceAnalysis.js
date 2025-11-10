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
import { useParams, useNavigate, useSearchParams } from "react-router-dom";
import { Alert, Spinner, Row, Col, Card, CardBody, Button } from "reactstrap";
import { fetchWithAuth, analyzeAwsResource, askAwsResourceQuestion } from "../services/api";
import PageHeader from "../components/layout/PageHeader";
import BaseAnalysis from "../components/common/BaseAnalysis";

const ResourceAnalysis = () => {
  const { id } = useParams();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  // Renamed for clarity and passed to BaseAnalysis
  const initialQuestionFromUrl = searchParams.get('question'); 
  
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [resource, setResource] = useState(null);
  const [analysisResult, setAnalysisResult] = useState(null);
  const [selectedWorkflow, setSelectedWorkflow] = useState(null);
  // const [analysisHistory, setAnalysisHistory] = useState([]); // REMOVED: Managed by BaseAnalysis
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
          console.log("Resource data received:", data);
          setResource(data);
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
  
  const fetchWorkflows = async (resourceType) => {
    try {
      console.log(`Fetching workflows for resource type: ${resourceType}`);
      setWorkflowsLoading(true);
      const backendUrl = process.env.REACT_APP_API_URL || "http://localhost:8080";
      console.log(`Using backend URL: ${backendUrl}`);
      const normalizedResourceType = resourceType.trim();
      console.log(`Normalized resource type: ${normalizedResourceType}`);
      console.log(`Making request to: /api/aws/analytics/workflows/${normalizedResourceType}`);
      const response = await fetchWithAuth(`/api/aws/analytics/workflows/${normalizedResourceType}`);
      console.log(`Workflow fetch response status: ${response.status}`);
      
      if (response.ok) {
        const data = await response.json();
        console.log(`Workflows fetched successfully:`, data);
        const validWorkflows = (data.workflows || []).map(workflow => {
          if (!workflow.id && workflow.workflow_id) {
            workflow.id = workflow.workflow_id;
          }
          return workflow;
        }).filter(workflow => workflow.id);
        console.log(`Valid workflows after processing:`, validWorkflows);
        setWorkflows(validWorkflows);
      } else {
        console.error(`Failed to fetch workflows - Status: ${response.status}`);
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
      setWorkflows([]);
    } finally {
      setWorkflowsLoading(false);
    }
  };
  
  // REMOVED useEffect for initialQuestion - BaseAnalysis will handle initiation
  // useEffect(() => {
  //   if (initialQuestionFromUrl && resource && !loading) {
  //     askRelatedQuestion(initialQuestionFromUrl, selectedWorkflow); // Or null if no workflow context for initial q
  //   }
  // }, [resource, initialQuestionFromUrl, loading, selectedWorkflow]);

  const runAnalysis = async (workflowId) => {
    if (!workflowId) {
      console.error("Cannot run analysis: Workflow ID is missing or invalid");
      setError("Cannot run analysis: Please select a valid workflow");
      return;
    }
    if (workflowId === selectedWorkflow && analysisResult) { // Avoid re-running if already selected and has a result
      console.log(`Workflow ${workflowId} is already selected and has results.`);
      return;
    }
    console.log(`Starting new analysis with workflow: ${workflowId}`);
    setAnalysisResult(null);
    // setAnalysisHistory([]); // REMOVED
    setError(null);
    setSelectedWorkflow(workflowId);
    setLoading(true);
    try {
      if (!resource || !resource.arn) {
        throw new Error("Resource ARN is missing or invalid");
      }
      console.log(`Running analysis with: Resource ARN=${resource.arn}, Workflow ID=${workflowId}`);
      const result = await analyzeAwsResource(resource.arn, workflowId);
      console.log("Analysis result received:", result);
      console.log("Result has related_questions:", result.related_questions);
      
      // Double check that related_questions are present
      if (!result.related_questions || result.related_questions.length === 0) {
        console.warn("No related_questions in result - adding default questions");
        result.related_questions = [
          "How can I optimize this resource further?",
          "What are the best practices for this type of resource?",
          "What metrics should I monitor for this resource?"
        ];
      }
      
      setAnalysisResult(result);
    } catch (err) {
      console.error("Analysis failed:", err);
      setError("Failed to run analysis: " + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  // Modified to accept workflowId from BaseAnalysis
  const askRelatedQuestion = async (question, workflowIdToUse) => {
    try {
      setLoading(true);
      console.log(`Asking related question: "${question}" for workflow: ${workflowIdToUse || 'None'}`);
      
      // Ensure resource ARN is available
      if (!resource || !resource.arn) {
        throw new Error("Resource ARN is not available. Cannot ask question.");
      }
      
      // Debug what's happening with the resource and workflow
      console.log("Resource details:", {
        id: resource.id,
        arn: resource.arn,
        type: resource.resource_type
      });
      
      // Check if this resource type supports questions
      const supportedQuestionTypes = ['EC2Instance', 'RdsInstance', 'DynamoDbTable'];
      
      if (!supportedQuestionTypes.includes(resource.resource_type)) {
        // Create a fallback response since the backend doesn't support questions for this resource type yet
                console.warn(`Resource type ${resource.resource_type} doesn't support questions yet. Using fallback response.`);
        
        const fallbackResult = {
          format: "markdown",
          content: `# ${question}\n\nI'm sorry, but detailed questions about ${resource.resource_type} resources are not supported yet. The backend currently only supports questions for EC2 instances, RDS instances, and DynamoDB tables.\n\nIn the meantime, you can use the initial analysis workflows to get basic information about your resource.`,
          related_questions: [
            "What are best practices for this resource type?",
            "How can I optimize this resource?",
            "What metrics should I monitor for this resource?"
          ],
          metadata: {
            timestamp: new Date().toISOString(),
            resource_type: resource.resource_type,
            workflow_type: workflowIdToUse || "question",
            data_sources: ["Frontend Fallback"]
          }
        };
        
        // Return the fallback result instead of calling the API
        return setAnalysisResult(fallbackResult);
      }
      
      // The workflowIdToUse is what BaseAnalysis thinks is active.
      // This could be the selectedWorkflow or null if it's an initial question from URL.
      // Use resource.arn for the API call as the backend expects ARN in the resource_id field
      const result = await askAwsResourceQuestion(resource.arn, question, workflowIdToUse);
      console.log("Question result received:", result);
      console.log("Question result has related_questions:", result.related_questions);
      
      // Double check that related_questions are present
      if (!result.related_questions || result.related_questions.length === 0) {
        console.warn("No related_questions in question result - adding default follow-up questions");
        result.related_questions = [
          "Can you explain more about this topic?",
          "What should I do next?",
          "How does this impact overall performance?"
        ];
      }
      
      setAnalysisResult(result);
      // Analysis history update removed, BaseAnalysis handles its qaHistory
    } catch (err) {
      setError("Failed to process question: " + (err.response?.data?.message || err.message));
      console.error("Error processing question:", err);
    } finally {
      setLoading(false);
    }
  };

  const getResourceTypeIcon = (resourceType) => {
    const iconMap = {
      "EC2Instance": "server", "RdsInstance": "database", "DynamoDbTable": "table",
      "S3Bucket": "archive", "ElasticacheCluster": "memory", "SqsQueue": "exchange",
      "KinesisStream": "stream", "LambdaFunction": "code", "ElasticLoadBalancer": "sitemap",
      "ApiGatewayApi": "plug", "CloudFrontDistribution": "globe", "Route53HostedZone": "route",
      "ElasticsearchDomain": "search", "ElasticBeanstalkEnvironment": "leaf", "EcsCluster": "cubes"
    };
    return iconMap[resourceType] || "cloud";
  };

  if (loading && !resource && !analysisResult) {
    return <div className="d-flex justify-content-center align-items-center" style={{ height: "300px" }}><Spinner color="primary" /></div>;
  }

  if (error && !resource) {
    return (
      <div>
        <PageHeader title="Resource Analysis" icon="fa-exclamation-triangle" breadcrumbs={[{ name: "Cloud", path: "/cloud" }, { name: "Resources", path: "/cloud" }, { name: "Analysis", active: true }]} />
        <Row><Col><Card><CardBody><Alert color="danger">
          <h4 className="alert-heading">Error Loading Resource</h4><p>{error}</p><hr />
          <p className="mb-0"><Button color="secondary" onClick={() => navigate("/cloud")}>Return to Resources</Button></p>
        </Alert></CardBody></Card></Col></Row>
      </div>
    );
  }

  return (
    <div>
      <PageHeader 
        title={`${formatResourceType(resource?.resource_type) || 'Resource'} Analysis`}
        icon={`fa-${getResourceTypeIcon(resource?.resource_type)}`}
        breadcrumbs={[
          { name: "Cloud", path: "/cloud" }, { name: "Resources", path: "/cloud" },
          { name: formatResourceType(resource?.resource_type) || "Resource", path: "/cloud" },
          { name: "Analysis", active: true }
        ]}
      />
      <Row>
        <Col lg={12}>
          <BaseAnalysis
            key={selectedWorkflow || initialQuestionFromUrl || 'initial'} // Ensure remount if initial question or workflow changes
            title={`${resource?.name || resource?.resource_id || 'Resource'} Analysis`}
            resource={resource}
            workflows={workflows}
            onRunAnalysis={runAnalysis}
            result={analysisResult}
            loading={loading || workflowsLoading}
            error={error}
            selectedWorkflow={selectedWorkflow}
            onAskQuestion={askRelatedQuestion}
            // analysisHistory={analysisHistory} // REMOVED
            initialQuestionFromUrl={initialQuestionFromUrl} // ADDED
          />
        </Col>
      </Row>
    </div>
  );
};

const formatResourceType = (resourceType) => {
  if (!resourceType) return '';
  switch (resourceType) {
    case 'EC2Instance': return 'EC2 Instance'; case 'RdsInstance': return 'RDS Instance';
    case 'DynamoDbTable': return 'DynamoDB Table'; case 'S3Bucket': return 'S3 Bucket';
    case 'ElasticacheCluster': return 'ElastiCache Cluster'; case 'SqsQueue': return 'SQS Queue';
    case 'KinesisStream': return 'Kinesis Stream'; case 'LambdaFunction': return 'Lambda Function';
    // ... other cases
    default: return resourceType.replace(/([A-Z])/g, ' $1').trim(); // Basic formatting for unlisted types
  }
};

export default ResourceAnalysis;
