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
import { Progress } from 'reactstrap';

/**
 * Component to visualize the progress through the 5-why analysis
 * 
 * @param {Object} props Component props
 * @param {number} props.currentDepth - The current depth of the analysis (0-5)
 * @param {number} props.maxDepth - The maximum depth of the analysis (default: 5)
 */
const FiveWhyProgress = ({ currentDepth, maxDepth = 5 }) => {
  // Calculate progress percentage
  const progressPercentage = Math.min(100, (currentDepth / maxDepth) * 100);
  
  // Determine progress color based on depth
  let progressColor = 'primary';
  if (currentDepth >= maxDepth) {
    progressColor = 'success';
  } else if (currentDepth >= Math.floor(maxDepth * 0.7)) {
    progressColor = 'info';
  }
  
  // Create step markers for the progress
  const steps = [];
  for (let i = 1; i <= maxDepth; i++) {
    steps.push(
      <div 
        key={i} 
        className={`progress-step ${i <= currentDepth ? 'active' : ''}`}
        style={{
          left: `${((i - 0.5) / maxDepth) * 100}%`,
          zIndex: 2
        }}
      >
        <div className="step-number">{i}</div>
        <div className="step-label">{i === 1 ? 'Initial' : i === maxDepth ? 'Root Cause' : `Level ${i}`}</div>
      </div>
    );
  }

  return (
    <div className="five-why-progress my-4">
      <div className="d-flex justify-content-between align-items-center mb-2">
        <h6 className="mb-0">5-Why Analysis Progress</h6>
        {currentDepth >= maxDepth && (
          <span className="badge bg-success">
            <i className="fas fa-check me-1"></i> Complete
          </span>
        )}
      </div>
      
      <div className="progress-container position-relative" style={{ height: '40px', marginBottom: '20px' }}>
        <Progress
          value={progressPercentage}
          color={progressColor}
          className="five-why-progress-bar"
          style={{ height: '10px', marginTop: '15px' }}
        />
        
        {steps}
      </div>
    </div>
  );
};

export default FiveWhyProgress;
