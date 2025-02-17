import React, { useState } from 'react';
import { CModal, CModalHeader, CModalTitle, CModalBody, CModalFooter, CButton, CForm, CFormLabel, CFormInput, CSpinner } from '@coreui/react';

const SqsModal = ({ show, handleClose, handleCreate }) => {
  const [queueName, setQueueName] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setLoading(true);
    await handleCreate(queueName);
    setLoading(false);
  };

  return (
    <CModal visible={show} onClose={handleClose} size="lg">
      <CModalHeader closeButton>
        <CModalTitle>Create SQS Queue</CModalTitle>
      </CModalHeader>
      <CModalBody>
        <CForm onSubmit={handleSubmit}>
          <div className="mb-3">
            <CFormLabel htmlFor="queueName">Queue Name</CFormLabel>
            <CFormInput
              type="text"
              id="queueName"
              value={queueName}
              onChange={(e) => setQueueName(e.target.value)}
              required
            />
          </div>
        </CForm>
      </CModalBody>
      <CModalFooter>
        {loading ? <CSpinner color="primary" /> : (
          <>
            <CButton color="primary" onClick={handleSubmit}>Create SQS Queue</CButton>{' '}
            <CButton color="secondary" onClick={handleClose}>Cancel</CButton>
          </>
        )}
      </CModalFooter>
    </CModal>
  );
};

export default SqsModal;
