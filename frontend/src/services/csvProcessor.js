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


import Papa from 'papaparse';

/**
 * Parses CSV content. Now returns an object where keys are from keyColumnName 
 * and values are the full row objects from the CSV.
 * @param {string} csvString The CSV data as a string.
 * @param {string} keyColumnName The name of the column to be used as the key.
 * @returns {object} An object (hash map) where keys are unique identifiers from keyColumnName 
 *                   and values are the corresponding full row objects.
 */
export const parseCSVData = (csvString, keyColumnName) => {
  const trimmedKeyColumn = keyColumnName.trim();
  const result = {};
  const parseResult = Papa.parse(csvString, {
    header: true,
    skipEmptyLines: true,
    dynamicTyping: true,
    transformHeader: header => header.trim(),
  });

  if (parseResult.errors.length > 0) {
    console.error("CSV Parsing Errors:", parseResult.errors);
  }

  parseResult.data.forEach(row => {
    const key = row[trimmedKeyColumn];
    if (key !== undefined && key !== null && key !== '') {
      result[key] = row; // Store the full row object
    } else {
      // console.warn("Skipping row due to missing or invalid key:", row);
    }
  });
  return result;
};

/**
 * Compares multiple CSV files, now including change type detection and selected include columns.
 * Files in filesData are assumed to be in the desired comparison order.
 * @param {Array<{name: string, content: string}>} filesData Array of file objects with name and content, in order.
 * @param {string} keyColumnName The name of the key column.
 * @param {string} valueColumnName The name of the value column.
 * @param {Array<string>} [includeColumns=[]] Array of additional column names to include in the result.
 * @returns {Array<object>} An array of objects representing the comparison result.
 */
export const compareMultipleCsvFiles = (filesData, keyColumnName, valueColumnName, includeColumns = []) => {
  const trimmedValueColumn = valueColumnName.trim();
  const individualFileFullData = filesData.map(file => {
    // parseCSVData now returns full row objects mapped by keyColumnName
    const parsedRowsMap = parseCSVData(file.content, keyColumnName);
    return {
      id: file.name,
      data: parsedRowsMap, // Map of key -> full row object
      keys: new Set(Object.keys(parsedRowsMap))
    };
  });

  const masterKeySet = new Set();
  individualFileFullData.forEach(fileData => {
    fileData.keys.forEach(key => masterKeySet.add(key));
  });

  const masterKeyList = Array.from(masterKeySet);
  masterKeyList.sort();

  const comparisonResult = masterKeyList.map(masterKey => {
    const row = { key: masterKey };
    let previousRowData = null; // Store the full row data from the previous file for this key
    let finalChangeType = '-';
    let lastSeenValue = undefined; // For the valueColumnName specifically
    let keyEverExists = false;

    // Populate included columns from the first file where the key appears
    // This ensures that if a key is new, its included column data is still captured.
    let populatedIncludeColumns = false;
    for (const col of includeColumns) {
      row[col] = null; // Initialize with null
    }

    for (let i = 0; i < individualFileFullData.length; i++) {
      const currentFileData = individualFileFullData[i];
      const currentRowDataForKey = currentFileData.data[masterKey]; // Full row object for this key, or undefined
      const currentValue = currentRowDataForKey ? currentRowDataForKey[trimmedValueColumn] : undefined;
      row[currentFileData.id] = currentValue !== undefined ? currentValue : null;

      // Populate includeColumns if not already done and key exists in current file
      if (!populatedIncludeColumns && currentRowDataForKey) {
        for (const col of includeColumns) {
          if (currentRowDataForKey.hasOwnProperty(col)) {
            row[col] = currentRowDataForKey[col];
          }
        }
        populatedIncludeColumns = true; // Mark as populated for this masterKey
      }

      const keyInCurrentFile = currentRowDataForKey !== undefined;

      if (i === 0) {
        if (keyInCurrentFile) {
          finalChangeType = filesData.length === 1 ? 'Base' : 'Unchanged';
          lastSeenValue = currentValue;
          keyEverExists = true;
          previousRowData = currentRowDataForKey;
        } else {
          finalChangeType = 'New'; // Will be 'New' when it first appears
        }
      } else {
        const keyInPreviousFile = individualFileFullData[i-1].keys.has(masterKey);
        if (keyInCurrentFile && !keyInPreviousFile) {
          finalChangeType = 'New';
          keyEverExists = true;
        } else if (!keyInCurrentFile && keyInPreviousFile) {
          finalChangeType = 'Deleted';
        } else if (keyInCurrentFile && keyInPreviousFile) {
          if (currentValue !== lastSeenValue) {
            finalChangeType = 'Changed';
          } else {
            // Check if any of the *included* columns changed, if the main value didn't
            let includedDataChanged = false;
            if (previousRowData && currentRowDataForKey) { // Ensure both previous and current row data exist
                for (const col of includeColumns) {
                    if (currentRowDataForKey[col] !== previousRowData[col]) {
                        includedDataChanged = true;
                        break;
                    }
                }
            }
            if (includedDataChanged) {
                finalChangeType = 'Changed';
            }
            // Only set to Unchanged if it wasn't already New or Changed from a prior state in the sequence
            else if (finalChangeType !== 'New' && finalChangeType !== 'Changed') {
              finalChangeType = 'Unchanged';
            }
          }
          keyEverExists = true;
        }
      }
      if (keyInCurrentFile) {
        lastSeenValue = currentValue;
        previousRowData = currentRowDataForKey; // Update previousRowData with current full row data
      }
    }

    row.changeType = finalChangeType;
    if (filesData.length === 1 && keyEverExists) {
      row.changeType = 'Base';
    } else if (filesData.length > 1 && !individualFileFullData[0].keys.has(masterKey) && keyEverExists) {
      if (row.changeType === '-' || row.changeType === 'Unchanged') {
        row.changeType = 'New';
      }
    }
    return row;
  });

  return comparisonResult;
};

/**
 * Compares data within a single CSV file, grouped by a date column.
 * Includes change type detection and selected include columns.
 * @param {string} csvString The CSV data as a string.
 * @param {string} dateColumnName The name of the date column for grouping.
 * @param {string} keyColumnName The name of the key column.
 * @param {string} valueColumnName The name of the value column.
 * @param {Array<string>} [includeColumns=[]] Array of additional column names to include in the result.
 * @returns {Array<object>} An array of objects representing the comparison result.
 */
export const compareSingleCsvFileByDate = (csvString, dateColumnName, keyColumnName, valueColumnName, includeColumns = []) => {
  const trimmedDateColumn = dateColumnName.trim();
  const trimmedKeyColumn = keyColumnName.trim();
  const trimmedValueColumn = valueColumnName.trim();

  const parseResult = Papa.parse(csvString, {
    header: true,
    skipEmptyLines: true,
    dynamicTyping: true,
    transformHeader: header => header.trim(),
  });

  if (parseResult.errors.length > 0) {
    console.error("CSV Parsing Errors:", parseResult.errors);
  }

  const groupedByDate = {};
  parseResult.data.forEach(row => {
    const dateValue = row[trimmedDateColumn];
    if (dateValue !== undefined && dateValue !== null) {
      if (!groupedByDate[dateValue]) {
        groupedByDate[dateValue] = [];
      }
      groupedByDate[dateValue].push(row); // Store full row
    }
  });

  const dateGroupsProcessed = Object.entries(groupedByDate).map(([date, rows]) => {
    const groupDataMap = {}; // key -> full row object
    rows.forEach(row => {
      const key = row[trimmedKeyColumn];
      if (key !== undefined && key !== null && key !== '') {
        groupDataMap[key] = row; // Store full row object
      }
    });
    return {
      id: String(date),
      data: groupDataMap, // Map of key -> full row object
      keys: new Set(Object.keys(groupDataMap))
    };
  });

  dateGroupsProcessed.sort((a, b) => new Date(a.id) - new Date(b.id));

  const masterKeySet = new Set();
  dateGroupsProcessed.forEach(group => {
    group.keys.forEach(key => masterKeySet.add(key));
  });
  
  const masterKeyList = Array.from(masterKeySet);
  masterKeyList.sort();

  const comparisonResult = masterKeyList.map(masterKey => {
    const row = { key: masterKey };
    let previousRowData = null; // Store the full row data from the previous date group
    let finalChangeType = '-';
    let lastSeenValue = undefined;
    let keyEverExistsInAnyDateGroup = false;

    // Initialize includeColumns with null
    for (const col of includeColumns) {
      row[col] = null;
    }
    let populatedIncludeColumns = false;

    for (let i = 0; i < dateGroupsProcessed.length; i++) {
      const currentDateGroup = dateGroupsProcessed[i];
      const currentRowDataForKey = currentDateGroup.data[masterKey]; // Full row object or undefined
      const currentValue = currentRowDataForKey ? currentRowDataForKey[trimmedValueColumn] : undefined;
      row[currentDateGroup.id] = currentValue !== undefined ? currentValue : null;

      if (!populatedIncludeColumns && currentRowDataForKey) {
        for (const col of includeColumns) {
          if (currentRowDataForKey.hasOwnProperty(col)) {
            row[col] = currentRowDataForKey[col];
          }
        }
        populatedIncludeColumns = true;
      }

      const keyInCurrentDate = currentRowDataForKey !== undefined;

      if (i === 0) {
        if (keyInCurrentDate) {
          finalChangeType = dateGroupsProcessed.length === 1 ? 'Base' : 'Unchanged';
          lastSeenValue = currentValue;
          keyEverExistsInAnyDateGroup = true;
          previousRowData = currentRowDataForKey;
        } else {
          finalChangeType = 'New';
        }
      } else {
        const keyInPreviousDate = dateGroupsProcessed[i-1].keys.has(masterKey);
        if (keyInCurrentDate && !keyInPreviousDate) {
          finalChangeType = 'New';
          keyEverExistsInAnyDateGroup = true;
        } else if (!keyInCurrentDate && keyInPreviousDate) {
          finalChangeType = 'Deleted';
        } else if (keyInCurrentDate && keyInPreviousDate) {
          if (currentValue !== lastSeenValue) {
            finalChangeType = 'Changed';
          } else {
            let includedDataChanged = false;
            if (previousRowData && currentRowDataForKey) {
                for (const col of includeColumns) {
                    if (currentRowDataForKey[col] !== previousRowData[col]) {
                        includedDataChanged = true;
                        break;
                    }
                }
            }
            if (includedDataChanged) {
                finalChangeType = 'Changed';
            }
            else if (finalChangeType !== 'New' && finalChangeType !== 'Changed') {
              finalChangeType = 'Unchanged';
            }
          }
          keyEverExistsInAnyDateGroup = true;
        }
      }
      if (keyInCurrentDate) {
        lastSeenValue = currentValue;
        previousRowData = currentRowDataForKey;
      }
    }
    row.changeType = finalChangeType;
    if (dateGroupsProcessed.length === 1 && keyEverExistsInAnyDateGroup) {
        row.changeType = 'Base';
    }

    return row;
  });

  return comparisonResult;
};

/**
 * Parses a CSV string and returns its headers and data rows for display.
 * @param {string} csvString The raw CSV data as a string.
 * @returns {{headers: string[], data: object[], error?: string}}
 */
export const parseCsvForTable = (csvString) => {
  if (!csvString || typeof csvString !== 'string') {
    return { headers: [], data: [], error: "CSV string is empty or invalid." };
  }

  const parseConfig = {
    header: true,
    skipEmptyLines: true,
    dynamicTyping: false, // Keep all data as strings for display consistency
    transformHeader: header => header.trim(), // Trim headers from CSV for display
  };
  const parsed = Papa.parse(csvString.trim(), parseConfig);

  if (parsed.errors && parsed.errors.length > 0) {
    console.error("Error parsing CSV for table display:", parsed.errors);
    // Return partial data if available, along with an error message
    return {
      headers: parsed.meta.fields || [],
      data: parsed.data || [],
      error: `Parsing issues: ${parsed.errors.map(e => e.message).join(", ")}`
    };
  }

  if (!parsed.data || parsed.data.length === 0 || !parsed.meta.fields || parsed.meta.fields.length === 0) {
    return { headers: [], data: [], error: "No data or headers found in CSV." };
  }

  return {
    headers: parsed.meta.fields,
    data: parsed.data,
  };
};

export const transposeCsvString = (csvString) => {
  if (!csvString || typeof csvString !== 'string') {
    return '';
  }

  // Parse the CSV into an array of arrays (rows and columns)
  // PapaParse without \`header: true\` gives an array of arrays.
  const parseConfig = {
    skipEmptyLines: true,
    dynamicTyping: false, // Keep everything as strings during initial parse for transposition
  };
  const parsed = Papa.parse(csvString.trim(), parseConfig);
  const originalData = parsed.data;

  if (!originalData || originalData.length === 0) {
    return '';
  }

  const numOriginalRows = originalData.length;
  const numOriginalCols = originalData[0] ? originalData[0].length : 0;

  if (numOriginalCols === 0) {
    return '';
  }

  const transposedData = [];
  for (let j = 0; j < numOriginalCols; j++) {
    transposedData[j] = [];
    for (let i = 0; i < numOriginalRows; i++) {
      transposedData[j][i] = originalData[i][j] !== undefined ? originalData[i][j] : '';
    }
  }

  // Convert the transposed array of arrays back to a CSV string
  return Papa.unparse(transposedData);
};
