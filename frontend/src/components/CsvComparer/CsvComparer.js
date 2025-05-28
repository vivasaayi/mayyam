import React, { useState, useCallback, useEffect } from 'react';
import Papa from 'papaparse';
import FileUpload from './FileUpload';
import ConfigPanel from './ConfigPanel';
import ResultsTable from './ResultsTable';
import SourceDataTable from './SourceDataTable';
import { compareMultipleCsvFiles, compareSingleCsvFileByDate, transposeCsvString, parseCsvForTable } from '../../services/csvProcessor';
import './csvComparer.css';

// Helper function to move an item in an array
const move = (array, fromIndex, toIndex) => {
  const newArray = [...array];
  const [item] = newArray.splice(fromIndex, 1);
  newArray.splice(toIndex, 0, item);
  return newArray;
};

const CsvComparer = () => {
  const [files, setFiles] = useState([]);
  const [keyColumn, setKeyColumn] = useState('');
  const [valueColumn, setValueColumn] = useState('');
  const [dateColumn, setDateColumn] = useState('');
  const [comparisonMode, setComparisonMode] = useState('multiple');
  const [comparisonResult, setComparisonResult] = useState(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState(null);
  const [columnHeaders, setColumnHeaders] = useState([]); // For display in ResultsTable
  const [fileValueColumnHeaders, setFileValueColumnHeaders] = useState([]); // For cell comparison logic in ResultsTable
  const [transposeSource, setTransposeSource] = useState(false);
  const [itemsPerPage, setItemsPerPage] = useState(10);
  const [sourceCurrentPage, setSourceCurrentPage] = useState(1);
  const [resultsCurrentPage, setResultsCurrentPage] = useState(1);
  const [sourceFileViews, setSourceFileViews] = useState([]);
  const [activeSourceTab, setActiveSourceTab] = useState(null);
  const [availableHeadersForSelection, setAvailableHeadersForSelection] = useState([]);
  const [selectedIncludeColumns, setSelectedIncludeColumns] = useState([]);

  const parseAndSetSourceViews = useCallback((currentFiles, shouldTranspose) => {
    const allHeaders = new Set();
    const views = currentFiles.map((file) => {
      const contentForDisplay = shouldTranspose ? transposeCsvString(file.originalContent) : file.originalContent;
      const parsedView = parseCsvForTable(contentForDisplay);
      if (parsedView.headers) {
        parsedView.headers.forEach(header => allHeaders.add(header));
      }
      return {
        id: file.id,
        name: file.name,
        headers: parsedView.headers,
        data: parsedView.data,
        error: parsedView.error,
      };
    });
    setSourceFileViews(views);
    setAvailableHeadersForSelection(Array.from(allHeaders));
    if (views.length > 0 && (!activeSourceTab || !views.find(v => v.id === activeSourceTab))) {
      setActiveSourceTab(views[0].id);
    }
    setSourceCurrentPage(1);
  }, [activeSourceTab]);

  useEffect(() => {
    parseAndSetSourceViews(files, transposeSource);
  }, [files, transposeSource, parseAndSetSourceViews]);

  const handleFilesSelected = useCallback((selectedFiles) => {
    setError(null);
    setComparisonResult(null);
    setResultsCurrentPage(1);
    setSelectedIncludeColumns([]);
    setAvailableHeadersForSelection([]);
    setColumnHeaders([]); // Reset display headers
    setFileValueColumnHeaders([]); // Reset file value headers

    const filePromises = Array.from(selectedFiles).map((file, index) => {
      return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = (e) => resolve({
          id: `file-${Date.now()}-${index}-${file.name}`,
          name: file.name,
          content: e.target.result,
          originalContent: e.target.result
        });
        reader.onerror = (e) => reject(e);
        reader.readAsText(file);
      });
    });

    Promise.all(filePromises)
      .then(fileContents => {
        // If comparisonMode is 'single', only keep the first file.
        if (comparisonMode === 'single' && fileContents.length > 0) {
          setFiles([fileContents[0]]);
        } else {
          setFiles(fileContents); // This will trigger the useEffect to parse and set source views
        }
      })
      .catch(err => {
        console.error("Error reading files:", err);
        setError("Error reading files. Please try again.");
        setFiles([]); // Clear files on error
      });
  }, [parseAndSetSourceViews, comparisonMode]);

  const handleIncludeColumnChange = (header) => {
    setSelectedIncludeColumns(prev => 
      prev.includes(header) 
        ? prev.filter(h => h !== header) 
        : [...prev, header]
    );
  };

  const handleProcess = () => {
    if ((comparisonMode === 'multiple' && files.length < 1) || (comparisonMode === 'single' && files.length !== 1)) {
      setError(comparisonMode === 'multiple' ? 'Please select at least one CSV file.' : 'Please select exactly one CSV file.');
      return;
    }
    if (!keyColumn || !valueColumn) {
      setError('Please specify both Key and Value column names.');
      return;
    }
    if (comparisonMode === 'single' && !dateColumn) {
      setError('Please specify the Date column name for single file comparison.');
      return;
    }

    setIsLoading(true);
    setError(null);
    setComparisonResult(null);
    setResultsCurrentPage(1);
    setColumnHeaders([]); // Clear old headers
    setFileValueColumnHeaders([]); // Clear old file value headers

    try {
      let processedFiles = files.map(file => ({
        ...file,
        content: transposeSource ? transposeCsvString(file.originalContent) : file.originalContent,
      }));

      let result;
      // Use a local copy of selectedIncludeColumns for this processing run
      const currentSelectedIncludeColumns = selectedIncludeColumns.filter(h => h !== keyColumn && h !== valueColumn && (comparisonMode === 'multiple' || h !== dateColumn));

      if (comparisonMode === 'multiple') {
        result = compareMultipleCsvFiles(processedFiles, keyColumn, valueColumn, currentSelectedIncludeColumns);
      } else {
        result = compareSingleCsvFileByDate(processedFiles[0].content, dateColumn, keyColumn, valueColumn, currentSelectedIncludeColumns);
      }

      if (result && result.length > 0) {
        const rawHeaders = Object.keys(result[0]);
        
        const newDisplayHeaders = [];
        const newFileValueHeaders = [];

        // 1. Key
        if (rawHeaders.includes('key')) newDisplayHeaders.push('key');
        // 2. ChangeType
        if (rawHeaders.includes('changeType')) newDisplayHeaders.push('changeType');
        // 3. Selected Include Columns (in the order they were selected)
        currentSelectedIncludeColumns.forEach(sc => {
            if (rawHeaders.includes(sc) && !newDisplayHeaders.includes(sc)) {
                newDisplayHeaders.push(sc);
            }
        });
        // 4. File/Date Value Columns (the rest of rawHeaders, maintaining their relative order from rawHeaders)
        rawHeaders.forEach(rh => {
            if (rh !== 'key' && rh !== 'changeType' && !currentSelectedIncludeColumns.includes(rh)) {
                if (!newDisplayHeaders.includes(rh)) { 
                    newDisplayHeaders.push(rh);
                }
                newFileValueHeaders.push(rh);
            }
        });

        setColumnHeaders(newDisplayHeaders);
        setFileValueColumnHeaders(newFileValueHeaders);
        setComparisonResult(result);
      } else {
        setComparisonResult([]);
        setColumnHeaders([]);
        setFileValueColumnHeaders([]);
        setError("No data to compare or columns not found. Please check your CSV and column names (especially if transposing).");
      }
    } catch (e) {
      console.error("Comparison Error:", e);
      setError(`Error during comparison: ${e.message}. Check console for details.`);
      setComparisonResult(null);
      setColumnHeaders([]);
      setFileValueColumnHeaders([]);
    } finally {
      setIsLoading(false);
    }
  };

  const handleModeChange = (newMode) => {
    setComparisonMode(newMode);
    setFiles([]);
    setComparisonResult(null);
    setResultsCurrentPage(1);
    setError(null);
    setKeyColumn('');
    setValueColumn('');
    setDateColumn('');
    setColumnHeaders([]);
    setFileValueColumnHeaders([]); // Reset file value headers
    setSelectedIncludeColumns([]);
    setAvailableHeadersForSelection([]);
  };

  const moveFile = useCallback((fromIndex, toIndex) => {
    setFiles(prevFiles => move(prevFiles, fromIndex, toIndex));
  }, []);

  // Calculate paginated data for source view
  const currentSourceView = sourceFileViews.find(view => view.id === activeSourceTab);
  const paginatedSourceData = currentSourceView && currentSourceView.data 
    ? currentSourceView.data.slice((sourceCurrentPage - 1) * itemsPerPage, sourceCurrentPage * itemsPerPage)
    : [];
  const sourceTotalPages = currentSourceView && currentSourceView.data 
    ? Math.ceil(currentSourceView.data.length / itemsPerPage) 
    : 0;

  // Calculate paginated data for results view
  const paginatedResultsData = comparisonResult 
    ? comparisonResult.slice((resultsCurrentPage - 1) * itemsPerPage, resultsCurrentPage * itemsPerPage)
    : [];
  const resultsTotalPages = comparisonResult 
    ? Math.ceil(comparisonResult.length / itemsPerPage) 
    : 0;

  const handleExportResults = () => {
    if (!comparisonResult || comparisonResult.length === 0) {
      console.warn("No results to export.");
      setError("No results to export.");
      return;
    }
    try {
      const csvString = Papa.unparse(comparisonResult, {
        columns: columnHeaders, 
        header: true,
      });
      const blob = new Blob([csvString], { type: 'text/csv;charset=utf-8;' });
      const link = document.createElement('a');
      const url = URL.createObjectURL(blob);
      link.setAttribute('href', url);
      link.setAttribute('download', 'comparison_results.csv');
      link.style.visibility = 'hidden';
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
      setError(null);
    } catch (exportError) {
      console.error("Error exporting CSV:", exportError);
      setError(`Error exporting CSV: ${exportError.message}`);
    }
  };

  return (
    <div className="csv-comparer">
      <h2>CSV File Comparer</h2>
      <ConfigPanel
        keyColumn={keyColumn}
        setKeyColumn={setKeyColumn}
        valueColumn={valueColumn}
        setValueColumn={setValueColumn}
        dateColumn={dateColumn}
        setDateColumn={setDateColumn}
        comparisonMode={comparisonMode}
        onModeChange={handleModeChange}
        transposeSource={transposeSource}
        setTransposeSource={setTransposeSource}
        availableHeaders={availableHeadersForSelection}
        selectedIncludeColumns={selectedIncludeColumns}
        onIncludeColumnChange={handleIncludeColumnChange}
        disabled={isLoading}
      />
      <FileUpload
        onFilesSelected={handleFilesSelected}
        disabled={isLoading}
        mode={comparisonMode}
      />

      {/* File Order Section - only for multiple files mode */}
      {comparisonMode === 'multiple' && files.length > 1 && (
        <div className="file-order-manager">
          <h4>Comparison Order (Drag to Reorder, or use buttons)</h4>
          <ul className="file-order-list">
            {files.map((file, index) => (
              <li key={file.id}>
                <span>{index + 1}. {file.name}</span>
                <div className="file-order-buttons">
                  {index > 0 && (
                    <button onClick={() => moveFile(index, index - 1)} disabled={isLoading} title="Move Up">
                      &#x25B2; {/* Up arrow */}
                    </button>
                  )}
                  {index < files.length - 1 && (
                    <button onClick={() => moveFile(index, index + 1)} disabled={isLoading} title="Move Down">
                      &#x25BC; {/* Down arrow */}
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Display Source CSV Data */}
      {sourceFileViews.length > 0 && (
        <div className="source-data-viewer">
          <h3>Source File Data Preview ({transposeSource ? "Transposed" : "Original"})</h3>
          {sourceFileViews.length > 1 && (
            <div className="tabs">
              {sourceFileViews.map((view) => (
                <button
                  key={view.id}
                  className={`tab-button ${activeSourceTab === view.id ? 'active' : ''}`}
                  onClick={() => setActiveSourceTab(view.id)}
                >
                  {view.name}
                </button>
              ))}
            </div>
          )}
          {sourceFileViews.map((view) => {
            if (sourceFileViews.length === 1 || activeSourceTab === view.id) {
              return (
                <div key={view.id} className="tab-content">
                  {view.error && <p className="error-message">{view.error}</p>}
                  <SourceDataTable
                    fileName={sourceFileViews.length > 1 ? view.name : null} // Show filename only if multiple tabs
                    headers={view.headers}
                    data={paginatedSourceData} // Pass paginated data
                    currentPage={sourceCurrentPage}
                    totalPages={sourceTotalPages}
                    onPageChange={setSourceCurrentPage}
                    itemsPerPage={itemsPerPage}
                    setItemsPerPage={setItemsPerPage} // Allow changing items per page
                    totalItems={currentSourceView ? currentSourceView.data.length : 0}
                  />
                </div>
              );
            }
            return null;
          })}
        </div>
      )}

      {files.length > 0 && comparisonMode === 'multiple' && files.length < 2 && (
         <div className="file-summary">
          <h4>Selected File for Comparison:</h4>
          <ul>
            {files.map(f => <li key={f.id}>{f.name}</li>)}
          </ul>
        </div>
      )}
      {/* Summary of files for single mode or when no reordering is shown */}
      { (comparisonMode === 'single' && files.length > 0) && (
         <div className="file-summary">
          <h4>Selected File for Comparison:</h4>
          <ul>
            {files.map(f => <li key={f.id}>{f.name}</li>)}
          </ul>
        </div>
      )}


      <button onClick={handleProcess} disabled={isLoading || files.length === 0}>
        {isLoading ? 'Processing...' : 'Process & Compare'}
      </button>

      {error && <p className="error-message">{error}</p>}

      {comparisonResult && (
        <>
          <ResultsTable
            data={paginatedResultsData} // Pass paginated data
            headers={columnHeaders} // Pass the already ordered headers
            fileValueColumnHeaders={fileValueColumnHeaders} // New prop: just the actual data columns for comparison
            mode={comparisonMode}
            currentPage={resultsCurrentPage}
            totalPages={resultsTotalPages}
            onPageChange={setResultsCurrentPage}
            itemsPerPage={itemsPerPage}
            setItemsPerPage={setItemsPerPage} // Allow changing items per page
            totalItems={comparisonResult ? comparisonResult.length : 0}
          />
          <button onClick={handleExportResults} className="export-button" disabled={isLoading || !comparisonResult || comparisonResult.length === 0}>
            Export Results as CSV
          </button>
        </>
      )}
    </div>
  );
};

export default CsvComparer;
