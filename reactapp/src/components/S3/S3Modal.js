import React, { useState } from 'react';
import { CModal, CModalHeader, CModalTitle, CModalBody, CModalFooter, CButton, CForm, CFormLabel, CFormInput, CSpinner } from '@coreui/react';

const S3Modal = ({ show, handleClose, handleCreate }) => {
  const [bucketName, setBucketName] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setLoading(true);
    await handleCreate(bucketName);
    setLoading(false);
  };

  return (
    <CModal visible={show} onClose={handleClose} size="lg">
      <CModalHeader closeButton>
        <CModalTitle>Create S3 Bucket</CModalTitle>
      </CModalHeader>
      <CModalBody>
        <CForm onSubmit={handleSubmit}>
          <div className="mb-3">
            <CFormLabel htmlFor="bucketName">Bucket Name</CFormLabel>
            <CFormInput
              type="text"
              id="bucketName"
              value={bucketName}
              onChange={(e) => setBucketName(e.target.value)}
              required
            />
          </div>
        </CForm>
      </CModalBody>
      <CModalFooter>
        {loading ? <CSpinner color="primary" /> : (
          <>
            <CButton color="primary" onClick={handleSubmit}>Create S3 Bucket</CButton>{' '}
            <CButton color="secondary" onClick={handleClose}>Cancel</CButton>
          </>
        )}
      </CModalFooter>
    </CModal>
  );
};

export default S3Modal;
