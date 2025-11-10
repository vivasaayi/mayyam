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

const ResultsTable = ({
  data,
  headers, // These are the display headers, already ordered by CsvComparer
  fileValueColumnHeaders, // New prop: Just the headers for file/date value columns
  mode,
  currentPage,
  totalPages,
  onPageChange,
  itemsPerPage,
  setItemsPerPage,
  totalItems
}) => {
  if (!data || data.length === 0) {
    return <p>No comparison data to display.</p>;
  }

  const getCellClass = (currentValue, previousValue) => {
    // This function now correctly compares only actual data values
    if (previousValue === undefined) return 'initial-value'; // First data column in the sequence
    if (currentValue === null && previousValue !== null) return 'deleted-value';
    if (currentValue !== null && previousValue === null) return 'added-value';
    if (currentValue !== previousValue) return 'changed-value';
    return 'no-change';
  };

  const handlePreviousPage = () => {
    if (currentPage > 1) {
      onPageChange(currentPage - 1);
    }
  };

  const handleNextPage = () => {
    if (currentPage < totalPages) {
      onPageChange(currentPage + 1);
    }
  };

  const handleItemsPerPageChange = (e) => {
    setItemsPerPage(Number(e.target.value));
  };

  return (
    <div className="results-table-container">
      <h4>Comparison Results</h4>
      <div className="pagination-controls">
        <span>Rows per page: </span>
        <select value={itemsPerPage} onChange={handleItemsPerPageChange}>
          <option value={10}>10</option>
          <option value={25}>25</option>
          <option value={50}>50</option>
          <option value={100}>100</option>
        </select>
        <button onClick={handlePreviousPage} disabled={currentPage === 1}>
          Previous
        </button>
        <span>
          Page {currentPage} of {totalPages} (Total: {totalItems} rows)
        </span>
        <button onClick={handleNextPage} disabled={currentPage === totalPages}>
          Next
        </button>
      </div>
      <div className="table-wrapper">
        <table>
          <thead>
            <tr>
              {headers.map(header => <th key={header}>{header.charAt(0).toUpperCase() + header.slice(1)}</th>)}
            </tr>
          </thead>
          <tbody>
            {data.map((row, rowIndex) => (
              <tr key={rowIndex}>
                {headers.map((header) => {
                  let cellClass = '';
                  // Check if the current header is one of the file/date value columns
                  if (fileValueColumnHeaders.includes(header)) {
                    const currentHeaderIndexInFileValues = fileValueColumnHeaders.indexOf(header);
                    let previousValueInSequence = undefined;
                    if (currentHeaderIndexInFileValues > 0) {
                      const previousFileValueHeader = fileValueColumnHeaders[currentHeaderIndexInFileValues - 1];
                      previousValueInSequence = row[previousFileValueHeader];
                    }
                    cellClass = getCellClass(row[header], previousValueInSequence);
                  } else if (header === 'changeType') {
                    // Optionally, style the changeType column based on its value
                    cellClass = `change-type-${String(row[header]).toLowerCase()}`;
                  }
                  // For other columns like 'key' or included static columns, no special class or default.

                  return <td key={`${rowIndex}-${header}`} className={cellClass}>{row[header] === null ? 'N/A' : String(row[header])}</td>;
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="legend">
        <h5>Legend:</h5>
        <span className="legend-item initial-value">Initial</span>
        <span className="legend-item added-value">Added</span>
        <span className="legend-item deleted-value">Deleted</span>
        <span className="legend-item changed-value">Changed</span>
        <span className="legend-item no-change">No Change</span>
      </div>
    </div>
  );
};

export default ResultsTable;
