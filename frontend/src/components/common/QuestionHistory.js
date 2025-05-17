import React from 'react';
import { Breadcrumb, BreadcrumbItem } from 'reactstrap';

/**
 * Displays the history of questions in the 5-why analysis cycle
 * 
 * @param {Object} props Component props
 * @param {Array} props.questions - Array of previously asked questions
 * @param {Function} props.onQuestionSelect - Function called when a question is selected
 * @param {number} props.activeIndex - Index of the currently active question
 */
const QuestionHistory = ({ questions, onQuestionSelect, activeIndex }) => {
  if (!questions || questions.length === 0) {
    return null;
  }

  return (
    <div className="question-history">
      <Breadcrumb className="question-path p-2 bg-light rounded">
        {questions.map((q, index) => (
          <BreadcrumbItem 
            key={index}
            active={index === activeIndex}
            onClick={() => onQuestionSelect(index, q)}
            className={index !== activeIndex ? "cursor-pointer" : ""}
            tag={index !== activeIndex ? "a" : "span"}
            style={{position: 'relative'}}
          >
            <span className="badge bg-primary rounded-pill me-1">{index + 1}</span>
            {q.length > 30 ? q.substring(0, 30) + '...' : q}
            {index !== activeIndex && (
              <span className="position-absolute top-100 start-50 translate-middle-x small text-primary" style={{whiteSpace: 'nowrap'}}>
                (click to view)
              </span>
            )}
          </BreadcrumbItem>
        ))}
      </Breadcrumb>
    </div>
  );
};

export default QuestionHistory;
