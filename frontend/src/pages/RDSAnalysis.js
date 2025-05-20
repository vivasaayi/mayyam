import React, { useState, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Alert, Spinner, Row, Col, Card, CardBody } from "reactstrap";
import { fetchWithAuth, analyzeRdsInstance, askRdsQuestion } from "../services/api";
import PageHeader from "../components/layout/PageHeader";
import BaseAnalysis from "../components/common/BaseAnalysis";

const ANALYSIS_WORKFLOWS = [
  { 
    id: "memory-usage", 
    name: "Analyze Memory Usage", 
    description: "Analyze memory utilization patterns and identify potential issues",
    icon: "fa-memory"
  },
  { 
    id: "cpu-usage", 
    name: "Analyze CPU Usage", 
    description: "Analyze CPU utilization patterns and bottlenecks",
    icon: "fa-microchip"
  },
  { 
    id: "disk-usage", 
    name: "Analyze Disk Usage", 
    description: "Analyze storage patterns, IOPS, and throughput",
    icon: "fa-hdd"
  },
  { 
    id: "performance", 
    name: "Performance Analysis", 
    description: "Comprehensive performance analysis of your RDS instance",
    icon: "fa-tachometer-alt"
  },
  { 
    id: "slow-queries", 
    name: "Slow Query Analysis", 
    description: "Find and analyze slow queries impacting performance",
    icon: "fa-database"
  }
];

// Mock related questions for demonstration
const RELATED_QUESTIONS = {
  "memory-usage": [
    "How can I optimize my memory configuration?",
    "Is my current memory allocation sufficient?",
    "What are the peak memory usage patterns?"
  ],
  "cpu-usage": [
    "Which queries are consuming the most CPU?",
    "How to scale my CPU resources efficiently?",
    "When do CPU spikes occur most frequently?"
  ],
  "disk-usage": [
    "What objects are taking up the most space?",
    "How can I improve IO performance?",
    "Should I consider storage autoscaling?"
  ],
  "performance": [
    "What is affecting my overall database performance?",
    "How do my current settings compare to best practices?",
    "What optimizations would provide the biggest performance gains?"
  ],
  "slow-queries": [
    "How can I optimize my slowest queries?",
    "What indexes should I add to improve performance?",
    "Are there query patterns that should be redesigned?"
  ]
};

const RDSAnalysis = () => {
  const { id } = useParams();
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [instance, setInstance] = useState(null);
  const [analysisResult, setAnalysisResult] = useState(null);
  const [selectedWorkflow, setSelectedWorkflow] = useState(null);
  const [analysisHistory, setAnalysisHistory] = useState([]);

  useEffect(() => {
    const fetchRdsInstance = async () => {
      try {
        setLoading(true);
        const response = await fetchWithAuth(`/api/aws/resources/${id}`);
        if (response.ok) {
          const data = await response.json();
          setInstance(data);
        } else {
          throw new Error("Failed to fetch RDS instance details");
        }
      } catch (err) {
        setError(err.message);
      } finally {
        setLoading(false);
      }
    };

    fetchRdsInstance();
  }, [id]);

  const runAnalysis = async (workflowId) => {
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
      // Call our backend API for real analysis
      const result = await analyzeRdsInstance(id, workflowId);
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
      const result = await askRdsQuestion(id, question, selectedWorkflow);
      
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

  if (loading && !instance && !analysisResult) {
    return (
      <div className="d-flex justify-content-center align-items-center" style={{ height: "300px" }}>
        <Spinner color="primary" />
      </div>
    );
  }

  return (
    <div>
      <PageHeader 
        title="RDS Analysis" 
        icon="fa-database" 
        breadcrumbs={[
          { name: "Cloud", path: "/cloud" },
          { name: "RDS Instances", path: "/cloud" },
          { name: "Analysis", active: true }
        ]}
      />

      <Row>
        {/* Main Analysis Panel - Full width now that Analysis Path is integrated */}
        <Col lg={12}>
          <BaseAnalysis
            key={selectedWorkflow || 'initial'} // Force remount when workflow changes
            title="RDS Instance Analysis"
            resource={instance}
            workflows={ANALYSIS_WORKFLOWS}
            onRunAnalysis={runAnalysis}
            result={analysisResult}
            loading={loading}
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

export default RDSAnalysis;
