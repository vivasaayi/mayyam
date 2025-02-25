import React, { useState } from 'react';
import { CModal, CModalHeader, CModalTitle, CModalBody, CModalFooter, CButton, CListGroup, CListGroupItem, CAlert, CSpinner } from '@coreui/react';

const DeleteConfirmationModal = ({ show, handleClose, handleConfirm, selectedStreams }) => {
  const [loading, setLoading] = useState(false);

  const handleDelete = async () => {
    setLoading(true);
    await handleConfirm();
    setLoading(false);
    handleClose();
  };

  return (
    <CModal visible={show} onClose={handleClose}>
      <CModalHeader closeButton>
        <CModalTitle>Confirm Deletion</CModalTitle>
      </CModalHeader>
      <CModalBody>
        <CAlert color="danger">
          Are you sure you want to delete the following tables? This action cannot be undone.
        </CAlert>
        <CListGroup>
          {selectedStreams.map((stream, index) => (
            <CListGroupItem key={index}>{stream.tableName}</CListGroupItem>
          ))}
        </CListGroup>
      </CModalBody>
      <CModalFooter>
        {loading ? <CSpinner color="danger" /> : (
          <>
            <CButton color="danger" onClick={handleDelete}>Delete</CButton>{' '}
            <CButton color="secondary" onClick={handleClose}>Cancel</CButton>
          </>
        )}
      </CModalFooter>
    </CModal>
  );
};

export default DeleteConfirmationModal;
