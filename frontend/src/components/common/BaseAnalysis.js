import React, { useState, useEffect } from "react";
import { 
  Card, CardHeader, CardBody, Button, Row, Col, ListGroup, ListGroupItem,
  Spinner, Alert, Badge
} from "reactstrap";
import ReactMarkdown from 'react-markdown';
import QuestionHistory from './QuestionHistory';
import FiveWhyProgress from './FiveWhyProgress';
import RelatedQuestionsPanel from './RelatedQuestionsPanel';
import FiveWhySummary from './FiveWhySummary';

/**
 * BaseAnalysis component that can be reused for different resource types
 * 
 * @param {Object} props Component props
 * @param {string} props.title - Title of the analysis page
 * @param {Object} props.resource - The resource being analyzed
 * @param {Array} props.workflows - Array of workflow objects with id, name, description, icon
 * @param {Function} props.onRunAnalysis - Function to call when a workflow is selected
 * @param {Object} props.result - The analysis result object
 * @param {boolean} props.loading - Loading state
 * @param {string} props.error - Error message
 */
const BaseAnalysis = ({
  title,
  resource,
  workflows,
  onRunAnalysis,
  result,
  loading,
  error,
  selectedWorkflow,
  onAskQuestion,
  analysisHistory
}) => {
  const [questionHistory, setQuestionHistory] = useState([]);
  const [currentQuestionIndex, setCurrentQuestionIndex] = useState(-1);
  const [analysisResults, setAnalysisResults] = useState([]);
  
  // Reset state when a new workflow is selected
  useEffect(() => {
    // When selectedWorkflow changes, reset our internal state
    if (selectedWorkflow) {
      console.log(`Workflow changed to: ${selectedWorkflow}, resetting analysis state`);
      setQuestionHistory([]);
      setAnalysisResults([]);
      setCurrentQuestionIndex(-1);
    }
  }, [selectedWorkflow]);
  
  // Update history when a new result comes in
  useEffect(() => {
    if (result && result.content) {
      // For the initial analysis, we'll use the workflow name as the "question"
      if (currentQuestionIndex === -1 && selectedWorkflow) {
        const workflowName = workflows.find(w => w.id === selectedWorkflow)?.name || selectedWorkflow;
        setQuestionHistory([workflowName]);
        setAnalysisResults([result]);
        setCurrentQuestionIndex(0);
      }
    }
  }, [result, selectedWorkflow, workflows, currentQuestionIndex]);
  
  // Handle asking a follow-up question
  const handleAskQuestion = (question) => {
    console.log(`BaseAnalysis: handleAskQuestion called with: ${question}`);
    
    // Don't proceed if we've already reached 5 questions
    if (questionHistory.length >= 5) {
      // Maybe show a toast notification instead of an alert
      alert('You have completed the 5-Why analysis cycle. Please review your findings or start a new analysis.');
      return;
    }
    
    // Add the question to history
    const newHistory = [...questionHistory, question];
    console.log(`Adding question to history. New length: ${newHistory.length}`);
    setQuestionHistory(newHistory);
    setCurrentQuestionIndex(newHistory.length - 1);
    
    // Call the parent's onAskQuestion function
    if (typeof onAskQuestion === 'function') {
      console.log(`Calling parent's onAskQuestion with: ${question}`);
      onAskQuestion(question);
    } else {
      console.error('onAskQuestion is not a function:', onAskQuestion);
    }
  };
  
  // Handle selecting a question from history
  const handleSelectQuestion = (index) => {
    if (index >= 0 && index < questionHistory.length) {
      setCurrentQuestionIndex(index);
      
      // If we're viewing a historical item and we have fewer than 5 questions,
      // show a prompt to continue from this point
      if (index < questionHistory.length - 1 && questionHistory.length < 5) {
        // This could be a toast notification instead
        const confirmContinue = window.confirm(
          "You're viewing a previous step in your analysis. Would you like to continue your analysis from this point instead? " +
          "(This will remove subsequent questions)"
        );
        
        if (confirmContinue) {
          // Trim the history to this point
          setQuestionHistory(questionHistory.slice(0, index + 1));
        }
      }
    }
  };
  
  // Update our results array when a new result comes in (for follow-up questions)
  useEffect(() => {
    if (result && result.content) {
      console.log("Received result:", result);
      
      if (questionHistory.length === 0 && selectedWorkflow) {
        // Initial analysis for a new workflow
        const workflowName = workflows.find(w => w.id === selectedWorkflow)?.name || selectedWorkflow;
        console.log(`Initial analysis: ${workflowName}`);
        setQuestionHistory([workflowName]);
        
        // If related_questions doesn't exist, add a default array
        const enhancedResult = {
          ...result,
          relatedQuestions: result.relatedQuestions || [
            "How can I optimize my memory configuration?",
            "Is my current memory allocation sufficient?",
            "What are the peak memory usage patterns?"
          ]
        };
        
        console.log("Enhanced result for initial analysis:", enhancedResult);
        setAnalysisResults([enhancedResult]);
        setCurrentQuestionIndex(0);
      } else if (currentQuestionIndex === questionHistory.length - 1) {
        // This is a follow-up question result
        console.log(`Follow-up result for question ${currentQuestionIndex + 1}`);
        
        // Ensure related questions exist
        const enhancedResult = {
          ...result,
          relatedQuestions: result.relatedQuestions || [
            "How does this compare to other similar workloads?",
            "What metrics should I monitor after applying these changes?",
            "How can I automate this optimization process?"
          ]
        };
        
        // Add to the results array
        let newResults;
        if (analysisResults.length <= currentQuestionIndex) {
          // If the array isn't long enough, create a new one with the right size
          newResults = [...analysisResults];
          // Fill any gaps
          while (newResults.length < currentQuestionIndex) {
            newResults.push(null);
          }
          newResults.push(enhancedResult);
        } else {
          // Just update the existing array
          newResults = [...analysisResults];
          newResults[currentQuestionIndex] = enhancedResult;
        }
        
        console.log("Updated analysis results:", newResults);
        setAnalysisResults(newResults);
      }
    }
  }, [result, questionHistory, currentQuestionIndex, selectedWorkflow, workflows]);

  if (loading && !result) {
    return (
      <div className="d-flex justify-content-center align-items-center" style={{ height: "300px" }}>
        <Spinner color="primary" />
      </div>
    );
  }

  return (
    <div>
      {error && <Alert color="danger">{error}</Alert>}

      {/* Analysis Workflows - Now at the top of the page */}
      <Card className="mb-4">
        <CardHeader>
          <h5 className="mb-0">Analysis Workflows</h5>
        </CardHeader>
        <CardBody className="p-3">
          <div className="workflow-grid">
            {workflows.map(workflow => (
              <Button 
                key={workflow.id}
                color={selectedWorkflow === workflow.id ? "primary" : "light"}
                className={`workflow-button p-3 mb-2 ${selectedWorkflow === workflow.id ? 'active' : ''}`}
                onClick={() => onRunAnalysis(workflow.id)}
              >
                <div className="d-flex align-items-center">
                  <div className="workflow-icon me-3">
                    <i className={`fas ${workflow.icon} fa-2x`}></i>
                  </div>
                  <div className="workflow-content">
                    <h6 className="mb-1">{workflow.name}</h6>
                    <small>{workflow.description}</small>
                  </div>
                </div>
              </Button>
            ))}
          </div>
        </CardBody>
      </Card>

      {/* Analysis Path - Now directly below Analysis Workflows */}
      {analysisResults.length > 0 && questionHistory.length > 0 && currentQuestionIndex >= 0 && (
        <Card className="mb-4 analysis-path-card">
          <CardHeader>
            <h5 className="mb-0">Analysis Path</h5>
          </CardHeader>
          <CardBody>
            <div className="current-path mb-3">
              <div className="d-flex align-items-center mb-2">
                <h6 className="mb-0 me-2">Current Path:</h6>
                <div className="progress flex-grow-1" style={{ height: '8px' }}>
                  <div 
                    className="progress-bar" 
                    role="progressbar" 
                    style={{ width: `${Math.min(100, (questionHistory.length / 5) * 100)}%` }}
                    aria-valuenow={questionHistory.length} 
                    aria-valuemin="0" 
                    aria-valuemax="5">
                  </div>
                </div>
                <span className="ms-2 text-muted small">{questionHistory.length}/5 Why</span>
              </div>
            </div>
            
            <QuestionHistory 
              questions={questionHistory} 
              onQuestionSelect={(index) => handleSelectQuestion(index)}
              activeIndex={currentQuestionIndex}
            />
          
            {/* 5-Why Progress Indicator */}
            <FiveWhyProgress 
              currentDepth={Math.min(questionHistory.length, 5)}
              maxDepth={5}
            />
          </CardBody>
        </Card>
      )}

      {/* Resource Details Card */}
      {resource && (
        <Card className="mb-4">
          <CardHeader>
            <h4 className="mb-0">
              <i className="fas fa-cube mr-2"></i>
              {resource.name || resource.identifier || "Resource"}
            </h4>
          </CardHeader>
          <CardBody>
            <Row>
              {Object.entries(resource)
                .filter(([key]) => !['id', 'name', 'identifier'].includes(key))
                .map(([key, value]) => (
                  <Col md={3} key={key}>
                    <p>
                      <strong>{key.replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase())}:</strong>{' '}
                      {typeof value === 'boolean' 
                        ? (value ? 'Yes' : 'No') 
                        : (typeof value === 'object' && value !== null 
                          ? JSON.stringify(value) 
                          : (value || 'N/A'))}
                    </p>
                  </Col>
                ))}
            </Row>
          </CardBody>
        </Card>
      )}

      <Row>
        <Col md={12}>
          {loading && result ? (
            <Card>
              <CardBody className="text-center p-5">
                <Spinner color="primary" />
                <p className="mt-3">Analyzing your resource...</p>
              </CardBody>
            </Card>
          ) : analysisResults.length > 0 && currentQuestionIndex >= 0 ? (
            <>
            
              <Card className="mb-4 analysis-card">
                <CardHeader className="d-flex justify-content-between align-items-center">
                  <h5 className="mb-0">
                    {currentQuestionIndex === 0 
                      ? workflows.find(w => w.id === selectedWorkflow)?.name
                      : (
                        <div className="d-flex align-items-center">
                          <i className="fas fa-question-circle me-2 text-primary"></i>
                          <span>{questionHistory[currentQuestionIndex]}</span>
                        </div>
                      )
                    }
                  </h5>
                  <Badge color="info" pill>
                    <i className="fas fa-info-circle me-1"></i>
                    {currentQuestionIndex === 0 ? "Initial Analysis" : `Follow-up ${currentQuestionIndex}`}
                  </Badge>
                </CardHeader>
                <CardBody>
                  <div className="analysis-content">
                    {analysisResults[currentQuestionIndex]?.format === "markdown" ? (
                      <ReactMarkdown>{analysisResults[currentQuestionIndex].content}</ReactMarkdown>
                    ) : analysisResults[currentQuestionIndex]?.format === "html" ? (
                      <div dangerouslySetInnerHTML={{ __html: analysisResults[currentQuestionIndex].content }} />
                    ) : (
                      <pre>{analysisResults[currentQuestionIndex]?.content}</pre>
                    )}
                  </div>
                </CardBody>
              </Card>
              
              {/* Debug info */}
              <div className="mb-2">
                <small className="text-muted">
                  <strong>Debug:</strong> Analysis Result: {analysisResults.length ? 'Available' : 'Missing'}, 
                  Related Questions: {analysisResults[currentQuestionIndex]?.relatedQuestions ? 
                    `${analysisResults[currentQuestionIndex].relatedQuestions.length} questions` : 'None'
                  }
                </small>
              </div>
              
              {/* Related Questions Section - Using RelatedQuestionsPanel */}
              {analysisResults[currentQuestionIndex]?.relatedQuestions && 
               analysisResults[currentQuestionIndex].relatedQuestions.length > 0 && 
               questionHistory.length < 5 && (
                <RelatedQuestionsPanel
                  questions={analysisResults[currentQuestionIndex].relatedQuestions}
                  onSelectQuestion={handleAskQuestion}
                  analysisDepth={questionHistory.length}
                />
              )}
              
              {/* Show summary when 5-why analysis is complete */}
              {questionHistory.length >= 5 && (
                <>
                  <div className="alert alert-success mb-4">
                    <h5><i className="fas fa-check-circle me-2"></i>5-Why Analysis Complete!</h5>
                    <p className="mb-0">You've reached the recommended depth for a 5-Why analysis. Review your findings below or start a new analysis workflow.</p>
                  </div>
                  
                  <FiveWhySummary 
                    analysisResults={analysisResults}
                    onExportResults={() => {
                      // For now, just create a simple text export
                      alert('Exporting analysis results will be implemented in a future version.');
                    }}
                  />
                </>
              )}
              
              {/* Fallback Questions Section (for debugging) */}
              {!analysisResults[currentQuestionIndex]?.relatedQuestions && 
               currentQuestionIndex === 0 && 
               questionHistory.length < 5 && (
                <div className="mb-4">
                  <Card className="border-warning">
                    <CardHeader className="bg-warning text-white">
                      <h5 className="mb-0">
                        <i className="fas fa-question-circle me-2"></i>
                        Sample Follow-up Questions
                      </h5>
                    </CardHeader>
                    <CardBody className="bg-light">
                      <p className="mb-3">
                        <strong>Debugging:</strong> These are sample questions since no related questions were found in the analysis result.
                      </p>
                      
                      <div className="related-questions-grid">
                        {["How can I optimize my memory configuration?", 
                          "Is my current memory allocation sufficient?", 
                          "What are the peak memory usage patterns?"].map((question, i) => (
                          <Card key={i} className="mb-2 related-question-card">
                            <CardBody>
                              <h6>{question}</h6>
                              <Button 
                                color="primary" 
                                size="sm" 
                                className="mt-2"
                                onClick={() => handleAskQuestion(question)}
                              >
                                <i className="fas fa-search-plus me-1"></i>
                                Analyze This
                              </Button>
                            </CardBody>
                          </Card>
                        ))}
                      </div>
                    </CardBody>
                  </Card>
                </div>
              )}
            </>
          ) : (
            <Card>
              <CardBody className="text-center p-5">
                <i className="fas fa-chart-line fa-3x mb-3 text-muted"></i>
                <h4>Start Your 5-Why Analysis</h4>
                <p className="text-muted mb-4">Choose one of the predefined analysis workflows above to begin analyzing your resource.</p>
                <div className="five-why-explanation bg-light p-3 rounded mx-auto" style={{ maxWidth: '600px' }}>
                  <h6><i className="fas fa-lightbulb me-2 text-warning"></i>What is 5-Why Analysis?</h6>
                  <p className="mb-0 text-start small">
                    The 5-Why technique is a simple but powerful tool for cutting quickly through the outward symptoms 
                    of a problem to reveal its underlying causes, so that you can deal with it once and for all.
                    Start by selecting a workflow, then follow the suggested questions to drill down to the root cause.
                  </p>
                </div>
              </CardBody>
            </Card>
          )}
        </Col>
      </Row>
    </div>
  );
};

export default BaseAnalysis;
