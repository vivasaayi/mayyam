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
