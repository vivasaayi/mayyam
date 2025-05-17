import React from 'react';
import { Card, CardHeader, CardBody } from 'reactstrap';

/**
 * Component to display related questions in a visually appealing way
 * 
 * @param {Object} props Component props
 * @param {Array} props.questions - Array of related questions
 * @param {Function} props.onSelectQuestion - Function called when a question is selected
 * @param {number} props.analysisDepth - Current depth of the analysis (1-5)
 */
const RelatedQuestionsPanel = ({ questions, onSelectQuestion, analysisDepth = 1 }) => {
  if (!questions || questions.length === 0) {
    return null;
  }

  return (
    <Card className="related-questions-panel">
      <CardHeader className="bg-light">
        <div className="d-flex justify-content-between align-items-center">
          <h5 className="mb-0">
            <i className="fas fa-lightbulb me-2 text-warning"></i>
            Follow-up Questions
          </h5>
          <div>
            {analysisDepth < 5 ? (
              <span className="badge bg-primary">
                {5 - analysisDepth} "Whys" remaining
              </span>
            ) : (
              <span className="badge bg-success">
                <i className="fas fa-check-circle me-1"></i>
                5-Why analysis complete
              </span>
            )}
          </div>
        </div>
      </CardHeader>
      {analysisDepth < 5 && (
        <div className="p-3 bg-light border-bottom">
          <div className="d-flex align-items-center">
            <i className="fas fa-info-circle text-primary me-2"></i>
            <span className="text-muted">Click on any question below to continue your 5-Why analysis and dig deeper.</span>
          </div>
        </div>
      )}
      <CardBody>
        <div className="related-questions-flow">
          {questions.map((question, index) => (
            <div 
              key={index}
              className="question-card"
              onClick={() => onSelectQuestion(question)}
            >
              <div className="question-number">{index + 1}</div>
              <div className="question-text">{question}</div>
              <div className="question-action">
                <span className="btn btn-sm btn-primary">
                  <i className="fas fa-arrow-right me-1"></i>
                  Ask
                </span>
              </div>
            </div>
          ))}
        </div>
      </CardBody>
    </Card>
  );
};

export default RelatedQuestionsPanel;
