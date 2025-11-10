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

const FileUpload = ({ onFilesSelected, disabled, mode }) => {
  const handleFileChange = (event) => {
    if (event.target.files) {
      onFilesSelected(event.target.files);
    }
  };

  return (
    <div className="file-upload-section">
      <label htmlFor="csv-upload">Upload CSV File(s):</label>
      <input 
        type="file" 
        id="csv-upload" 
        accept=".csv" 
        multiple={mode === 'multiple'} 
        onChange={handleFileChange} 
        disabled={disabled}
      />
      {mode === 'single' && <p><small>Select exactly one file for single file comparison mode.</small></p>}
      {mode === 'multiple' && <p><small>Select one or more files for multiple file comparison mode.</small></p>}
    </div>
  );
};

export default FileUpload;
