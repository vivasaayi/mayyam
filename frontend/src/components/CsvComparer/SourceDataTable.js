import React from 'react';

const SourceDataTable = ({
  data,
  headers,
  fileName,
  currentPage,
  totalPages,
  onPageChange,
  itemsPerPage,
  setItemsPerPage,
  totalItems
}) => {
  if (!data || data.length === 0 || !headers || headers.length === 0) {
    return (
      <div className="source-data-table-container">
        {fileName && <h4>Source Data: {fileName}</h4>}
        <p>
          {fileName ? `No data to display for ${fileName}. ` : 'No data to display. ' }
          Check if the file is empty, if headers are missing, or if all rows were empty.
        </p>
      </div>
    );
  }

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
    <div className="source-data-table-container">
      {fileName && <h4>Source Data: {fileName}</h4>}
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
      <div className="table-wrapper"> {/* Reusing existing .table-wrapper for consistent styling */}
        <table>
          <thead>
            <tr>
              {headers.map(header => <th key={header}>{header}</th>)}
            </tr>
          </thead>
          <tbody>
            {data.map((row, rowIndex) => (
              <tr key={rowIndex}>
                {headers.map(header => (
                  <td key={`${rowIndex}-${header}`}>
                    {row[header] !== undefined && row[header] !== null ? String(row[header]) : ''}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
};

export default SourceDataTable;
