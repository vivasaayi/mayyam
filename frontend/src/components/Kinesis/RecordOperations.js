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


import React, { useState, useEffect } from 'react';
import {
  CCard,
  CCardHeader,
  CCardBody,
  CRow,
  CCol,
  CForm,
  CFormInput,
  CFormTextarea,
  CFormSelect,
  CButton,
  CAlert,
  CSpinner,
  CBadge,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CInputGroup,
  CInputGroupText,
  CButtonGroup
} from '@coreui/react';
import { FaDatabase, FaUpload, FaDownload, FaPlay, FaStop, FaSync } from 'react-icons/fa';
import KinesisService from '../../services/kinesisService';

const RecordOperations = ({ profile, region, streamName, shards }) => {
  const [activeTab, setActiveTab] = useState('put');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Put Record State
  const [putRecord, setPutRecord] = useState({
    data: '',
    partitionKey: '',
    explicitHashKey: '',
    sequenceNumberForOrdering: ''
  });
  const [putRecords, setPutRecords] = useState([
    { data: '', partitionKey: '' }
  ]);

  // Get Records State
  const [getRecords, setGetRecords] = useState({
    shardId: '',
    shardIteratorType: 'TRIM_HORIZON',
    sequenceNumber: '',
    timestamp: '',
    limit: 100
  });
  const [shardIterator, setShardIterator] = useState('');
  const [retrievedRecords, setRetrievedRecords] = useState([]);
  const [autoFetch, setAutoFetch] = useState(false);
  const [fetchInterval, setFetchInterval] = useState(null);

  // Clear messages
  const clearMessages = () => {
    setError(null);
    setSuccess(null);
  };

  // Put single record
  const handlePutRecord = async (e) => {
    e.preventDefault();
    if (!putRecord.data.trim() || !putRecord.partitionKey.trim()) {
      setError('Data and Partition Key are required');
      return;
    }

    setLoading(true);
    clearMessages();

    try {
      const request = {
        stream_name: streamName,
        data: putRecord.data,
        partition_key: putRecord.partitionKey
      };

      if (putRecord.explicitHashKey) {
        request.explicit_hash_key = putRecord.explicitHashKey;
      }
      if (putRecord.sequenceNumberForOrdering) {
        request.sequence_number_for_ordering = putRecord.sequenceNumberForOrdering;
      }

      const response = await KinesisService.putRecord(profile, region, request);
      setSuccess(`Record added successfully. Shard: ${response.shard_id}, Sequence: ${response.sequence_number}`);
      setPutRecord({ data: '', partitionKey: '', explicitHashKey: '', sequenceNumberForOrdering: '' });
    } catch (err) {
      setError(`Failed to put record: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Put multiple records
  const handlePutRecords = async (e) => {
    e.preventDefault();
    const validRecords = putRecords.filter(r => r.data.trim() && r.partitionKey.trim());
    
    if (validRecords.length === 0) {
      setError('At least one record with data and partition key is required');
      return;
    }

    setLoading(true);
    clearMessages();

    try {
      const request = {
        stream_name: streamName,
        records: validRecords.map(record => ({
          data: record.data,
          partition_key: record.partitionKey
        }))
      };

      const response = await KinesisService.putRecords(profile, region, request);
      const successCount = response.records.filter(r => !r.error_code).length;
      const failureCount = response.records.filter(r => r.error_code).length;
      
      setSuccess(`${successCount} records added successfully${failureCount > 0 ? `, ${failureCount} failed` : ''}`);
      setPutRecords([{ data: '', partitionKey: '' }]);
    } catch (err) {
      setError(`Failed to put records: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Get shard iterator
  const getShardIteratorForRecords = async () => {
    if (!getRecords.shardId) {
      setError('Please select a shard');
      return;
    }

    setLoading(true);
    clearMessages();

    try {
      const request = {
        stream_name: streamName,
        shard_id: getRecords.shardId,
        shard_iterator_type: getRecords.shardIteratorType
      };

      if (getRecords.shardIteratorType === 'AT_SEQUENCE_NUMBER' || 
          getRecords.shardIteratorType === 'AFTER_SEQUENCE_NUMBER') {
        if (!getRecords.sequenceNumber) {
          setError('Sequence number is required for this iterator type');
          setLoading(false);
          return;
        }
        request.starting_sequence_number = getRecords.sequenceNumber;
      }

      if (getRecords.shardIteratorType === 'AT_TIMESTAMP') {
        if (!getRecords.timestamp) {
          setError('Timestamp is required for this iterator type');
          setLoading(false);
          return;
        }
        request.timestamp = getRecords.timestamp;
      }

      const response = await KinesisService.getShardIterator(profile, region, request);
      setShardIterator(response.shard_iterator);
      setSuccess('Shard iterator obtained successfully');
    } catch (err) {
      setError(`Failed to get shard iterator: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Fetch records
  const fetchRecords = async () => {
    if (!shardIterator) {
      setError('Please get a shard iterator first');
      return;
    }

    setLoading(true);
    clearMessages();

    try {
      const request = {
        shard_iterator: shardIterator,
        limit: getRecords.limit
      };

      const response = await KinesisService.getRecords(profile, region, request);
      setRetrievedRecords(prevRecords => [...prevRecords, ...response.records]);
      setShardIterator(response.next_shard_iterator);
      setSuccess(`Retrieved ${response.records.length} records`);
    } catch (err) {
      setError(`Failed to get records: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Toggle auto-fetch
  const toggleAutoFetch = () => {
    if (autoFetch) {
      if (fetchInterval) {
        clearInterval(fetchInterval);
        setFetchInterval(null);
      }
      setAutoFetch(false);
    } else {
      if (!shardIterator) {
        setError('Please get a shard iterator first');
        return;
      }
      const interval = setInterval(fetchRecords, 5000); // Fetch every 5 seconds
      setFetchInterval(interval);
      setAutoFetch(true);
    }
  };

  // Add record to put records array
  const addPutRecord = () => {
    setPutRecords([...putRecords, { data: '', partitionKey: '' }]);
  };

  // Remove record from put records array
  const removePutRecord = (index) => {
    setPutRecords(putRecords.filter((_, i) => i !== index));
  };

  // Update put record
  const updatePutRecord = (index, field, value) => {
    const updated = [...putRecords];
    updated[index][field] = value;
    setPutRecords(updated);
  };

  // Clear retrieved records
  const clearRetrievedRecords = () => {
    setRetrievedRecords([]);
    setShardIterator('');
  };

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (fetchInterval) {
        clearInterval(fetchInterval);
      }
    };
  }, [fetchInterval]);

  // Shard iterator types
  const iteratorTypes = [
    { value: 'TRIM_HORIZON', label: 'Trim Horizon (oldest)' },
    { value: 'LATEST', label: 'Latest (newest)' },
    { value: 'AT_SEQUENCE_NUMBER', label: 'At Sequence Number' },
    { value: 'AFTER_SEQUENCE_NUMBER', label: 'After Sequence Number' },
    { value: 'AT_TIMESTAMP', label: 'At Timestamp' }
  ];

  return (
    <div>
      {/* Alert Messages */}
      {error && (
        <CAlert color="danger" dismissible onClose={clearMessages}>
          {error}
        </CAlert>
      )}
      {success && (
        <CAlert color="success" dismissible onClose={clearMessages}>
          {success}
        </CAlert>
      )}

      {/* Navigation Tabs */}
      <CCard>
        <CCardHeader>
          <CNav variant="tabs">
            <CNavItem>
              <CNavLink
                active={activeTab === 'put'}
                onClick={() => setActiveTab('put')}
                style={{ cursor: 'pointer' }}
              >
                <FaUpload className="me-2" />Put Records
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink
                active={activeTab === 'putBatch'}
                onClick={() => setActiveTab('putBatch')}
                style={{ cursor: 'pointer' }}
              >
                <FaUpload className="me-2" />Put Records (Batch)
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink
                active={activeTab === 'get'}
                onClick={() => setActiveTab('get')}
                style={{ cursor: 'pointer' }}
              >
                <FaDownload className="me-2" />Get Records
              </CNavLink>
            </CNavItem>
          </CNav>
        </CCardHeader>

        <CCardBody>
          <CTabContent>
            {/* Put Single Record Tab */}
            <CTabPane visible={activeTab === 'put'}>
              <CForm onSubmit={handlePutRecord}>
                <CRow>
                  <CCol md={6}>
                    <CFormTextarea
                      label="Record Data"
                      value={putRecord.data}
                      onChange={(e) => setPutRecord({...putRecord, data: e.target.value})}
                      placeholder="Enter your record data here..."
                      rows={4}
                      required
                    />
                  </CCol>
                  <CCol md={6}>
                    <CFormInput
                      label="Partition Key"
                      value={putRecord.partitionKey}
                      onChange={(e) => setPutRecord({...putRecord, partitionKey: e.target.value})}
                      placeholder="partition-key"
                      required
                    />
                    <CFormInput
                      label="Explicit Hash Key (optional)"
                      value={putRecord.explicitHashKey}
                      onChange={(e) => setPutRecord({...putRecord, explicitHashKey: e.target.value})}
                      placeholder="0-340282366920938463463374607431768211455"
                      className="mt-2"
                    />
                    <CFormInput
                      label="Sequence Number for Ordering (optional)"
                      value={putRecord.sequenceNumberForOrdering}
                      onChange={(e) => setPutRecord({...putRecord, sequenceNumberForOrdering: e.target.value})}
                      placeholder="sequence-number"
                      className="mt-2"
                    />
                  </CCol>
                </CRow>
                <CRow className="mt-3">
                  <CCol>
                    <CButton type="submit" color="primary" disabled={loading}>
                      {loading ? <CSpinner size="sm" /> : <FaUpload />}
                      {loading ? ' Putting Record...' : ' Put Record'}
                    </CButton>
                  </CCol>
                </CRow>
              </CForm>
            </CTabPane>

            {/* Put Multiple Records Tab */}
            <CTabPane visible={activeTab === 'putBatch'}>
              <CForm onSubmit={handlePutRecords}>
                {putRecords.map((record, index) => (
                  <CCard key={index} className="mb-3">
                    <CCardBody>
                      <CRow>
                        <CCol md={5}>
                          <CFormTextarea
                            label={`Record ${index + 1} Data`}
                            value={record.data}
                            onChange={(e) => updatePutRecord(index, 'data', e.target.value)}
                            placeholder="Enter record data..."
                            rows={3}
                          />
                        </CCol>
                        <CCol md={5}>
                          <CFormInput
                            label={`Record ${index + 1} Partition Key`}
                            value={record.partitionKey}
                            onChange={(e) => updatePutRecord(index, 'partitionKey', e.target.value)}
                            placeholder="partition-key"
                          />
                        </CCol>
                        <CCol md={2} className="d-flex align-items-end">
                          {putRecords.length > 1 && (
                            <CButton
                              color="danger"
                              size="sm"
                              onClick={() => removePutRecord(index)}
                            >
                              Remove
                            </CButton>
                          )}
                        </CCol>
                      </CRow>
                    </CCardBody>
                  </CCard>
                ))}
                
                <CRow>
                  <CCol>
                    <CButtonGroup>
                      <CButton
                        type="button"
                        color="secondary"
                        onClick={addPutRecord}
                      >
                        Add Record
                      </CButton>
                      <CButton type="submit" color="primary" disabled={loading}>
                        {loading ? <CSpinner size="sm" /> : <FaUpload />}
                        {loading ? ' Putting Records...' : ' Put Records'}
                      </CButton>
                    </CButtonGroup>
                  </CCol>
                </CRow>
              </CForm>
            </CTabPane>

            {/* Get Records Tab */}
            <CTabPane visible={activeTab === 'get'}>
              <CRow>
                <CCol md={6}>
                  <CCard>
                    <CCardHeader>Shard Iterator Configuration</CCardHeader>
                    <CCardBody>
                      <CFormSelect
                        label="Shard"
                        value={getRecords.shardId}
                        onChange={(e) => setGetRecords({...getRecords, shardId: e.target.value})}
                      >
                        <option value="">Select a shard</option>
                        {shards.map(shard => (
                          <option key={shard.shard_id} value={shard.shard_id}>
                            {shard.shard_id}
                          </option>
                        ))}
                      </CFormSelect>

                      <CFormSelect
                        label="Iterator Type"
                        value={getRecords.shardIteratorType}
                        onChange={(e) => setGetRecords({...getRecords, shardIteratorType: e.target.value})}
                        className="mt-2"
                      >
                        {iteratorTypes.map(type => (
                          <option key={type.value} value={type.value}>
                            {type.label}
                          </option>
                        ))}
                      </CFormSelect>

                      {(getRecords.shardIteratorType === 'AT_SEQUENCE_NUMBER' || 
                        getRecords.shardIteratorType === 'AFTER_SEQUENCE_NUMBER') && (
                        <CFormInput
                          label="Starting Sequence Number"
                          value={getRecords.sequenceNumber}
                          onChange={(e) => setGetRecords({...getRecords, sequenceNumber: e.target.value})}
                          placeholder="sequence-number"
                          className="mt-2"
                        />
                      )}

                      {getRecords.shardIteratorType === 'AT_TIMESTAMP' && (
                        <CFormInput
                          label="Timestamp"
                          type="datetime-local"
                          value={getRecords.timestamp}
                          onChange={(e) => setGetRecords({...getRecords, timestamp: e.target.value})}
                          className="mt-2"
                        />
                      )}

                      <CFormInput
                        label="Limit"
                        type="number"
                        min="1"
                        max="10000"
                        value={getRecords.limit}
                        onChange={(e) => setGetRecords({...getRecords, limit: parseInt(e.target.value)})}
                        className="mt-2"
                      />

                      <div className="mt-3">
                        <CButton
                          color="primary"
                          onClick={getShardIteratorForRecords}
                          disabled={loading}
                          className="me-2"
                        >
                          Get Shard Iterator
                        </CButton>
                      </div>

                      {shardIterator && (
                        <div className="mt-2">
                          <CBadge color="success">Iterator Ready</CBadge>
                        </div>
                      )}
                    </CCardBody>
                  </CCard>
                </CCol>

                <CCol md={6}>
                  <CCard>
                    <CCardHeader>Fetch Records</CCardHeader>
                    <CCardBody>
                      <CButtonGroup>
                        <CButton
                          color="primary"
                          onClick={fetchRecords}
                          disabled={loading || !shardIterator}
                        >
                          {loading ? <CSpinner size="sm" /> : <FaDownload />}
                          {loading ? ' Fetching...' : ' Fetch Records'}
                        </CButton>
                        <CButton
                          color={autoFetch ? "success" : "outline-success"}
                          onClick={toggleAutoFetch}
                          disabled={!shardIterator}
                        >
                          {autoFetch ? <FaStop /> : <FaPlay />}
                          {autoFetch ? ' Stop Auto' : ' Auto Fetch'}
                        </CButton>
                        <CButton
                          color="secondary"
                          onClick={clearRetrievedRecords}
                        >
                          Clear Records
                        </CButton>
                      </CButtonGroup>

                      {autoFetch && (
                        <div className="mt-2">
                          <CBadge color="success">Auto-fetching every 5 seconds</CBadge>
                        </div>
                      )}

                      <div className="mt-3">
                        <strong>Retrieved Records: {retrievedRecords.length}</strong>
                      </div>
                    </CCardBody>
                  </CCard>
                </CCol>
              </CRow>

              {/* Retrieved Records Table */}
              {retrievedRecords.length > 0 && (
                <CCard className="mt-3">
                  <CCardHeader>Retrieved Records</CCardHeader>
                  <CCardBody>
                    <div style={{ maxHeight: '400px', overflowY: 'auto' }}>
                      <CTable responsive hover>
                        <CTableHead>
                          <CTableRow>
                            <CTableHeaderCell>Sequence Number</CTableHeaderCell>
                            <CTableHeaderCell>Partition Key</CTableHeaderCell>
                            <CTableHeaderCell>Data</CTableHeaderCell>
                            <CTableHeaderCell>Timestamp</CTableHeaderCell>
                          </CTableRow>
                        </CTableHead>
                        <CTableBody>
                          {retrievedRecords.map((record, index) => (
                            <CTableRow key={index}>
                              <CTableDataCell>
                                <small>{record.sequence_number}</small>
                              </CTableDataCell>
                              <CTableDataCell>{record.partition_key}</CTableDataCell>
                              <CTableDataCell>
                                <div style={{ maxWidth: '200px', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                                  {KinesisService.decodeData(record.data)}
                                </div>
                              </CTableDataCell>
                              <CTableDataCell>
                                <small>{new Date(record.approximate_arrival_timestamp).toLocaleString()}</small>
                              </CTableDataCell>
                            </CTableRow>
                          ))}
                        </CTableBody>
                      </CTable>
                    </div>
                  </CCardBody>
                </CCard>
              )}
            </CTabPane>
          </CTabContent>
        </CCardBody>
      </CCard>
    </div>
  );
};

export default RecordOperations;
