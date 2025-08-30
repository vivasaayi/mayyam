import React from 'react';
import { Card, CardHeader, CardBody, Spinner } from 'reactstrap';

/**
 * Component to display related questions in a visually appealing way
 * 
 * @param {Object} props Component props
 * @param {Array} props.relatedQuestions - Array of related questions
 * @param {Function} props.onAskQuestion - Function called when a question is selected
 * @param {number} props.currentDepth - Current depth of the analysis (1-5)
 * @param {boolean} props.isLoading - Whether an analysis is currently loading (passed from BaseAnalysis)
 * @param {string|null} props.askingQuestionText - The text of the question currently being asked (passed from BaseAnalysis)
 */
const RelatedQuestionsPanel = ({ relatedQuestions, onAskQuestion, currentDepth = 1, isLoading, askingQuestionText }) => {
  if (!relatedQuestions || relatedQuestions.length === 0) {
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
            {currentDepth < 5 ? (
              <span className="badge bg-primary">
                {5 - currentDepth} 'Whys' remaining
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
      {currentDepth < 5 && (
        <div className="p-3 bg-light border-bottom">
          <div className="d-flex align-items-center">
            <i className="fas fa-info-circle text-primary me-2"></i>
            <span className="text-muted">Click on any question below to continue your 5-Why analysis and dig deeper.</span>
          </div>
        </div>
      )}
      <CardBody>
        <div className="related-questions-flow">
          {relatedQuestions.map((question, index) => {
            const isCurrentlyAskingThis = isLoading && askingQuestionText === question;
            const cardIsDisabled = !!isLoading; 

            return (
              <div 
                key={index}
                className={`question-card ${cardIsDisabled ? 'disabled-card' : ''} ${isCurrentlyAskingThis ? 'asking' : ''}`}
                onClick={() => !cardIsDisabled && onAskQuestion(question)}
                style={{ 
                  cursor: cardIsDisabled ? 'not-allowed' : 'pointer',
                  opacity: cardIsDisabled && !isCurrentlyAskingThis ? 0.7 : 1 
                }}
              >
                <div className="question-number">{index + 1}</div>
                <div className="question-text">{question}</div>
                <div className="question-action">
                  {isCurrentlyAskingThis ? (
                    <span className="btn btn-sm btn-info">
                      <Spinner size="sm" type="grow" color="light" className="me-1" />
                      Asking...
                    </span>
                  ) : cardIsDisabled ? (
                     <span className="btn btn-sm btn-secondary"> 
                      <i className="fas fa-hourglass-half me-1"></i>
                      Wait
                    </span>
                  ) :(
                    <span className="btn btn-sm btn-primary">
                      <i className="fas fa-arrow-right me-1"></i>
                      Ask
                    </span>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </CardBody>
    </Card>
  );
};

export default RelatedQuestionsPanel;
