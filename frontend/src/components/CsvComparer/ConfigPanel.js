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

const ConfigPanel = ({
  keyColumn,
  setKeyColumn,
  valueColumn,
  setValueColumn,
  dateColumn,
  setDateColumn,
  comparisonMode,
  onModeChange,
  transposeSource,
  setTransposeSource,
  availableHeaders, // New prop
  selectedIncludeColumns, // New prop
  onIncludeColumnChange, // New prop
  disabled
}) => {
  return (
    <div className="config-panel">
      <h4>Configuration</h4>
      <div className="mode-selector">
        <label>
          <input 
            type="radio" 
            value="multiple" 
            checked={comparisonMode === 'multiple'} 
            onChange={() => onModeChange('multiple')} 
            disabled={disabled}
          />
          Compare Multiple Files
        </label>
        <label>
          <input 
            type="radio" 
            value="single" 
            checked={comparisonMode === 'single'} 
            onChange={() => onModeChange('single')} 
            disabled={disabled}
          />
          Compare Single File (Group by Date)
        </label>
      </div>
      <div className="column-inputs">
        <div>
          <label htmlFor="key-column">Key Column Name:</label>
          <input 
            type="text" 
            id="key-column" 
            value={keyColumn}
            onChange={(e) => setKeyColumn(e.target.value)} 
            placeholder="e.g., ResourceID, UserID"
            disabled={disabled}
          />
        </div>
        <div>
          <label htmlFor="value-column">Value Column Name:</label>
          <input 
            type="text" 
            id="value-column" 
            value={valueColumn}
            onChange={(e) => setValueColumn(e.target.value)} 
            placeholder="e.g., Cost, MetricValue"
            disabled={disabled}
          />
        </div>
        {comparisonMode === 'single' && (
          <div>
            <label htmlFor="date-column">Date Column Name:</label>
            <input 
              type="text" 
              id="date-column" 
              value={dateColumn}
              onChange={(e) => setDateColumn(e.target.value)} 
              placeholder="e.g., Timestamp, ReportDate"
              disabled={disabled}
            />
          </div>
        )}
      </div>
      <div className="transpose-option">
        <label>
          <input
            type="checkbox"
            checked={transposeSource}
            onChange={(e) => setTransposeSource(e.target.checked)}
            disabled={disabled}
          />
          Transpose source CSV file(s) before processing
        </label>
        <small>
            If checked, rows become columns and columns become rows.
            The first column of the original CSV will typically become the header row of the transposed CSV.
            Your Key/Value/Date column names should then refer to the headers of this *transposed* structure.
        </small>
      </div>

      {availableHeaders && availableHeaders.length > 0 && (
        <div className="include-columns-panel">
          <h5>Include Additional Columns in Output:</h5>
          <div className="checkbox-group">
            {availableHeaders.map(header => (
              <label key={header} className="checkbox-label">
                <input
                  type="checkbox"
                  value={header}
                  checked={selectedIncludeColumns.includes(header)}
                  onChange={() => onIncludeColumnChange(header)}
                  disabled={disabled || header === keyColumn || header === valueColumn || (comparisonMode === 'single' && header === dateColumn)}
                />
                {header}
              </label>
            ))}
          </div>
          <small>Selected columns will be added to the results table and export.</small>
        </div>
      )}
    </div>
  );
};

export default ConfigPanel;
