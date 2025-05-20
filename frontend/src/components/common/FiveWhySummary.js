import React from 'react';
import { Card, CardHeader, CardBody, Button } from 'reactstrap';

/**
 * Component to summarize the insights from the 5-why analysis cycle
 * 
 * @param {Object} props Component props
 * @param {Array} props.analysisResults - Array of analysis results from the 5-why cycle
 * @param {Function} props.onExportResults - Function to export or share results
 */
const FiveWhySummary = ({ analysisResults, onExportResults }) => {
  if (!analysisResults || analysisResults.length < 5) {
    return null; // Only show summary when 5-why cycle is complete
  }

  // Extract key findings from the 5-why analysis
  const initialProblem = analysisResults[0]?.title || "Initial Analysis";
  const rootCause = analysisResults[analysisResults.length - 1]?.title || "Root Cause Analysis";
  
  // Create a timeline of the analysis steps
  const analysisPath = Array.isArray(analysisResults) ? analysisResults.map((result, index) => {
    // Try to extract a title from the content
    const titleMatch = result?.content?.match(/^# (.+?)$/m);
    const title = titleMatch ? titleMatch[1] : `Analysis Step ${index + 1}`;
    return { title, result };
  }) : [];

  return (
    <Card className="five-why-summary mt-4">
      <CardHeader className="bg-success text-white">
        <h5 className="mb-0">
          <i className="fas fa-check-circle me-2"></i>
          5-Why Analysis Complete
        </h5>
      </CardHeader>
      <CardBody>
        <div className="summary-content">
          <div className="summary-journey">
            <div className="journey-start">
              <h6>Initial Problem</h6>
              <p>{typeof initialProblem === 'object' ? JSON.stringify(initialProblem) : initialProblem}</p>
            </div>
            <div className="journey-path">
              <i className="fas fa-arrow-right"></i>
              <i className="fas fa-arrow-right"></i>
              <i className="fas fa-arrow-right"></i>
            </div>
            <div className="journey-end">
              <h6>Root Cause</h6>
              <p>{typeof rootCause === 'object' ? JSON.stringify(rootCause) : rootCause}</p>
            </div>
          </div>
          
          {/* Analysis Steps Timeline */}
          <div className="analysis-timeline mt-5">
            <h6 className="mb-3">Your Analysis Path</h6>
            <div className="timeline-container">
              {analysisPath.map((step, index) => (
                <div className="timeline-item" key={index}>
                  <div className="timeline-marker">{index + 1}</div>
                  <div className="timeline-content">
                    <h6>{typeof step.title === 'object' ? JSON.stringify(step.title) : step.title}</h6>
                    <p className="text-muted small">
                      {/* Extract first recommendation or finding if available */}
                      {typeof step.result.content === 'string' 
                        ? step.result.content.split('\n').slice(2, 3).join('').substring(0, 80) + '...'
                        : 'Content not available'}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </div>
          
          <div className="summary-actions mt-4">
            <h6>Recommended Next Steps</h6>
            <ul>
              <li>Review the detailed findings from each analysis step</li>
              <li>Create a remediation plan based on the root cause</li>
              <li>Set up monitoring to prevent similar issues in the future</li>
            </ul>
            
            <div className="d-flex justify-content-center gap-3 mt-4">
              <Button color="primary" onClick={onExportResults}>
                <i className="fas fa-file-export me-2"></i>
                Export Analysis Results
              </Button>
              <Button color="success" onClick={() => window.print()}>
                <i className="fas fa-print me-2"></i>
                Print Report
              </Button>
            </div>
          </div>
        </div>
      </CardBody>
    </Card>
  );
};

export default FiveWhySummary;
