import React from 'react';
import { CModal, CModalHeader, CModalTitle, CModalBody, CModalFooter, CButton } from '@coreui/react';

const DeleteConfirmationModal = ({ show, handleClose, handleConfirm, selectedStreams }) => {
  return (
    <CModal show={show} onClose={handleClose}>
      <CModalHeader closeButton>
        <CModalTitle>Confirm Deletion</CModalTitle>
      </CModalHeader>
      <CModalBody>
        <p className="text-danger">
          <i className="cil-warning" style={{ fontSize: '2rem' }}></i> Are you sure you want to delete the selected ElastiCache clusters?
        </p>
        <ul>
          {selectedStreams.map((stream, index) => (
            <li key={index}>{stream.cacheClusterId}</li>
          ))}
        </ul>
      </CModalBody>
      <CModalFooter>
        <CButton color="danger" onClick={handleConfirm}>Delete</CButton>{' '}
        <CButton color="secondary" onClick={handleClose}>Cancel</CButton>
      </CModalFooter>
    </CModal>
  );
};

export default DeleteConfirmationModal;