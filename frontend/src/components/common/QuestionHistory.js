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
      <Breadcrumb className="question-path p-2 bg-light rounded" listClassName="mb-0">
        {questions.map((q, index) => (
          <BreadcrumbItem 
            key={index}
            active={index === activeIndex}
            onClick={index !== activeIndex ? () => onQuestionSelect(index, q) : undefined}
            tag={index !== activeIndex ? "a" : "span"}
            href={index !== activeIndex ? "#" : undefined} 
            className={index !== activeIndex ? "cursor-pointer" : ""}
            title={q}
          >
            <span className={`badge rounded-pill me-2 ${index === activeIndex ? 'bg-secondary' : 'bg-primary'}`}>{index + 1}</span>
            {q.length > 40 ? q.substring(0, 40) + '...' : q}
          </BreadcrumbItem>
        ))}
      </Breadcrumb>
    </div>
  );
};

export default QuestionHistory;
