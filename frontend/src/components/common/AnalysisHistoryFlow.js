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


import React from 'react';
import { Card, CardHeader, CardBody, Button } from 'reactstrap';

/**
 * Component to visualize the analysis history as a flow chart
 * 
 * @param {Object} props Component props
 * @param {Array} props.history - Array of analysis history items
 * @param {Function} props.onHistoryItemClick - Function called when a history item is clicked
 * @param {number} props.activeIndex - Index of the currently active history item
 */
const AnalysisHistoryFlow = ({ history, onHistoryItemClick, activeIndex }) => {
  if (!history || history.length === 0) {
    return null;
  }

  return (
    <Card className="mb-4">
      <CardHeader>
        <h5 className="mb-0">
          <i className="fas fa-project-diagram me-2"></i>
          Analysis Path
        </h5>
      </CardHeader>
      <CardBody>
        <div className="analysis-flow-chart">
          {history.map((item, index) => (
            <div 
              key={index}
              className={`analysis-node ${index === activeIndex ? 'active' : ''}`}
            >
              <div className="node-number">{index + 1}</div>
              <div 
                className="node-content"
                onClick={() => onHistoryItemClick(index)}
              >
                <div className="node-title">{item.question}</div>
                <div className="node-timestamp">
                  {new Date(item.timestamp).toLocaleTimeString()}
                </div>
              </div>
              {index < history.length - 1 && (
                <div className="node-connector">
                  <i className="fas fa-arrow-down"></i>
                </div>
              )}
            </div>
          ))}
        </div>
        
        {history.length < 5 && (
          <div className="text-center mt-3">
            <Button color="primary" size="sm" disabled>
              <i className="fas fa-info-circle me-1"></i>
              Continue asking follow-up questions to reach root cause
            </Button>
          </div>
        )}
      </CardBody>
    </Card>
  );
};

export default AnalysisHistoryFlow;
