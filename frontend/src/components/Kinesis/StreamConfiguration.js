import React, { useState, useEffect } from 'react';
import {
  CCard,
  CCardHeader,
  CCardBody,
  CRow,
  CCol,
  CForm,
  CFormInput,
  CFormSelect,
  CFormSwitch,
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
  CModal,
  CModalHeader,
  CModalTitle,
  CModalBody,
  CModalFooter,
  CInputGroup,
  CInputGroupText
} from '@coreui/react';
import { FaCog, FaSave, FaShieldAlt, FaTags, FaClock, FaDatabase, FaEye } from 'react-icons/fa';
import KinesisService from '../../services/kinesisService';

const StreamConfiguration = ({ profile, region, stream, onUpdate }) => {
  const [activeTab, setActiveTab] = useState('retention');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  // Retention Configuration
  const [retentionHours, setRetentionHours] = useState(stream.retention_period_hours || 24);
  
  // Encryption Configuration
  const [encryptionType, setEncryptionType] = useState(stream.encryption_type || 'NONE');
  const [kmsKeyId, setKmsKeyId] = useState(stream.key_id || '');
  
  // Enhanced Monitoring
  const [enhancedMonitoring, setEnhancedMonitoring] = useState(false);
  const [shardLevelMetrics, setShardLevelMetrics] = useState([]);
  
  // Tags
  const [tags, setTags] = useState([]);
  const [showAddTagModal, setShowAddTagModal] = useState(false);
  const [newTag, setNewTag] = useState({ key: '', value: '' });

  // Clear messages
  const clearMessages = () => {
    setError(null);
    setSuccess(null);
  };

  // Load current configuration
  const loadConfiguration = async () => {
    try {
      // Load enhanced monitoring status if available
      // This would typically come from describe-stream API
      setEnhancedMonitoring(stream.enhanced_monitoring?.length > 0 || false);
      setShardLevelMetrics(stream.enhanced_monitoring || []);
      
      // Load tags - in a real implementation, this would be a separate API call
      const storedTags = JSON.parse(localStorage.getItem(`kinesis_tags_${stream.name}`) || '[]');
      setTags(storedTags);
    } catch (err) {
      console.error('Failed to load configuration:', err);
    }
  };

  // Update retention period
  const updateRetentionPeriod = async () => {
    if (retentionHours < 24 || retentionHours > 8760) {
      setError('Retention period must be between 24 and 8760 hours');
      return;
    }

    setLoading(true);
    clearMessages();

    try {
      if (retentionHours > stream.retention_period_hours) {
        await KinesisService.increaseStreamRetentionPeriod(
          profile,
          region,
          stream.name,
          retentionHours
        );
      } else {
        await KinesisService.decreaseStreamRetentionPeriod(
          profile,
          region,
          stream.name,
          retentionHours
        );
      }
      
      setSuccess(`Retention period updated to ${retentionHours} hours`);
      onUpdate();
    } catch (err) {
      setError(`Failed to update retention period: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Enable/Disable enhanced monitoring
  const toggleEnhancedMonitoring = async () => {
    setLoading(true);
    clearMessages();

    try {
      const metrics = ['IncomingRecords', 'OutgoingRecords', 'IncomingBytes', 'OutgoingBytes'];
      
      if (enhancedMonitoring) {
        await KinesisService.disableEnhancedMonitoring(
          profile,
          region,
          stream.name,
          metrics
        );
        setEnhancedMonitoring(false);
        setShardLevelMetrics([]);
        setSuccess('Enhanced monitoring disabled');
      } else {
        await KinesisService.enableEnhancedMonitoring(
          profile,
          region,
          stream.name,
          metrics
        );
        setEnhancedMonitoring(true);
        setShardLevelMetrics(metrics);
        setSuccess('Enhanced monitoring enabled');
      }
      
      onUpdate();
    } catch (err) {
      setError(`Failed to toggle enhanced monitoring: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Add tag
  const addTag = () => {
    if (!newTag.key.trim() || !newTag.value.trim()) {
      setError('Tag key and value are required');
      return;
    }

    if (tags.some(tag => tag.key === newTag.key)) {
      setError('Tag key already exists');
      return;
    }

    const updatedTags = [...tags, { ...newTag }];
    setTags(updatedTags);
    localStorage.setItem(`kinesis_tags_${stream.name}`, JSON.stringify(updatedTags));
    setNewTag({ key: '', value: '' });
    setShowAddTagModal(false);
    setSuccess('Tag added successfully');
  };

  // Remove tag
  const removeTag = (keyToRemove) => {
    const updatedTags = tags.filter(tag => tag.key !== keyToRemove);
    setTags(updatedTags);
    localStorage.setItem(`kinesis_tags_${stream.name}`, JSON.stringify(updatedTags));
    setSuccess('Tag removed successfully');
  };

  // Update shard count (for provisioned mode)
  const updateShardCount = async (newShardCount) => {
    if (stream.stream_mode_details?.stream_mode !== 'PROVISIONED') {
      setError('Shard count can only be updated for provisioned streams');
      return;
    }

    const currentShardCount = stream.shards ? stream.shards.length : 0;
    if (newShardCount === currentShardCount) {
      return;
    }

    setLoading(true);
    clearMessages();

    try {
      await KinesisService.updateShardCount(
        profile,
        region,
        stream.name,
        newShardCount
      );
      
      setSuccess(`Shard count update initiated. Target: ${newShardCount} shards`);
      onUpdate();
    } catch (err) {
      setError(`Failed to update shard count: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadConfiguration();
  }, [stream]);

  // Available encryption types
  const encryptionTypes = [
    { value: 'NONE', label: 'No encryption' },
    { value: 'KMS', label: 'AWS KMS encryption' }
  ];

  // Stream mode badge
  const getStreamModeBadge = (mode) => {
    return (
      <CBadge color={mode === 'ON_DEMAND' ? 'info' : 'primary'}>
        {mode || 'PROVISIONED'}
      </CBadge>
    );
  };

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

      {/* Stream Overview */}
      <CCard className="mb-3">
        <CCardHeader>Stream Overview</CCardHeader>
        <CCardBody>
          <CRow>
            <CCol md={3}>
              <strong>Stream Name:</strong><br />
              {stream.name}
            </CCol>
            <CCol md={3}>
              <strong>Status:</strong><br />
              <CBadge color="success">{stream.stream_status}</CBadge>
            </CCol>
            <CCol md={3}>
              <strong>Mode:</strong><br />
              {getStreamModeBadge(stream.stream_mode_details?.stream_mode)}
            </CCol>
            <CCol md={3}>
              <strong>Shard Count:</strong><br />
              {stream.shards ? stream.shards.length : 0}
            </CCol>
          </CRow>
        </CCardBody>
      </CCard>

      {/* Configuration Tabs */}
      <CCard>
        <CCardHeader>
          <CNav variant="tabs">
            <CNavItem>
              <CNavLink
                active={activeTab === 'retention'}
                onClick={() => setActiveTab('retention')}
                style={{ cursor: 'pointer' }}
              >
                <FaClock className="me-2" />Retention
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink
                active={activeTab === 'encryption'}
                onClick={() => setActiveTab('encryption')}
                style={{ cursor: 'pointer' }}
              >
                <FaShieldAlt className="me-2" />Encryption
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink
                active={activeTab === 'monitoring'}
                onClick={() => setActiveTab('monitoring')}
                style={{ cursor: 'pointer' }}
              >
                <FaEye className="me-2" />Monitoring
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink
                active={activeTab === 'scaling'}
                onClick={() => setActiveTab('scaling')}
                style={{ cursor: 'pointer' }}
              >
                <FaDatabase className="me-2" />Scaling
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink
                active={activeTab === 'tags'}
                onClick={() => setActiveTab('tags')}
                style={{ cursor: 'pointer' }}
              >
                <FaTags className="me-2" />Tags
              </CNavLink>
            </CNavItem>
          </CNav>
        </CCardHeader>

        <CCardBody>
          <CTabContent>
            {/* Retention Tab */}
            <CTabPane visible={activeTab === 'retention'}>
              <CForm>
                <CRow>
                  <CCol md={6}>
                    <CInputGroup>
                      <CFormInput
                        type="number"
                        min="24"
                        max="8760"
                        value={retentionHours}
                        onChange={(e) => setRetentionHours(parseInt(e.target.value))}
                        placeholder="24"
                      />
                      <CInputGroupText>hours</CInputGroupText>
                    </CInputGroup>
                    <small className="text-muted">
                      Retention period (24 hours to 365 days)
                    </small>
                  </CCol>
                  <CCol md={6} className="d-flex align-items-start">
                    <CButton
                      color="primary"
                      onClick={updateRetentionPeriod}
                      disabled={loading || retentionHours === stream.retention_period_hours}
                    >
                      {loading ? <CSpinner size="sm" /> : <FaSave />}
                      {loading ? ' Updating...' : ' Update Retention'}
                    </CButton>
                  </CCol>
                </CRow>
                
                <div className="mt-3">
                  <h6>Current Configuration</h6>
                  <p><strong>Current Retention:</strong> {stream.retention_period_hours || 24} hours</p>
                  <p><strong>Data Loss Warning:</strong> Decreasing retention period may result in data loss for records older than the new retention period.</p>
                </div>
              </CForm>
            </CTabPane>

            {/* Encryption Tab */}
            <CTabPane visible={activeTab === 'encryption'}>
              <CForm>
                <CRow>
                  <CCol md={6}>
                    <CFormSelect
                      label="Encryption Type"
                      value={encryptionType}
                      onChange={(e) => setEncryptionType(e.target.value)}
                      disabled={true} // Encryption cannot be modified after stream creation
                    >
                      {encryptionTypes.map(type => (
                        <option key={type.value} value={type.value}>
                          {type.label}
                        </option>
                      ))}
                    </CFormSelect>
                    <small className="text-muted">
                      Encryption type cannot be modified after stream creation
                    </small>
                  </CCol>
                  {encryptionType === 'KMS' && (
                    <CCol md={6}>
                      <CFormInput
                        label="KMS Key ID"
                        value={kmsKeyId}
                        disabled={true}
                        placeholder="alias/aws/kinesis"
                      />
                    </CCol>
                  )}
                </CRow>
                
                <div className="mt-3">
                  <h6>Current Configuration</h6>
                  <p><strong>Encryption:</strong> {stream.encryption_type || 'NONE'}</p>
                  {stream.key_id && (
                    <p><strong>KMS Key:</strong> {stream.key_id}</p>
                  )}
                </div>
              </CForm>
            </CTabPane>

            {/* Monitoring Tab */}
            <CTabPane visible={activeTab === 'monitoring'}>
              <CForm>
                <CRow>
                  <CCol md={12}>
                    <CFormSwitch
                      label="Enhanced (Shard-level) Monitoring"
                      checked={enhancedMonitoring}
                      onChange={toggleEnhancedMonitoring}
                      disabled={loading}
                    />
                    <small className="text-muted">
                      Enable shard-level metrics in CloudWatch (additional charges apply)
                    </small>
                  </CCol>
                </CRow>

                {enhancedMonitoring && (
                  <div className="mt-3">
                    <h6>Enabled Metrics</h6>
                    <div className="d-flex flex-wrap gap-2">
                      {shardLevelMetrics.map(metric => (
                        <CBadge key={metric} color="info">{metric}</CBadge>
                      ))}
                    </div>
                  </div>
                )}

                <div className="mt-3">
                  <h6>Monitoring Information</h6>
                  <p><strong>Basic Monitoring:</strong> Stream-level metrics (free)</p>
                  <p><strong>Enhanced Monitoring:</strong> Shard-level metrics (additional cost)</p>
                  <p><strong>CloudWatch Logs:</strong> Not currently configured</p>
                </div>
              </CForm>
            </CTabPane>

            {/* Scaling Tab */}
            <CTabPane visible={activeTab === 'scaling'}>
              <CForm>
                <CRow>
                  <CCol md={6}>
                    <h6>Current Configuration</h6>
                    <p><strong>Stream Mode:</strong> {getStreamModeBadge(stream.stream_mode_details?.stream_mode)}</p>
                    <p><strong>Current Shards:</strong> {stream.shards ? stream.shards.length : 0}</p>
                    
                    {stream.stream_mode_details?.stream_mode === 'PROVISIONED' && (
                      <>
                        <CFormInput
                          type="number"
                          min="1"
                          max="10000"
                          label="Target Shard Count"
                          placeholder={stream.shards ? stream.shards.length.toString() : '1'}
                          onBlur={(e) => {
                            const newCount = parseInt(e.target.value);
                            if (newCount && newCount !== (stream.shards ? stream.shards.length : 0)) {
                              updateShardCount(newCount);
                            }
                          }}
                        />
                        <small className="text-muted">
                          Scaling operations may take several minutes to complete
                        </small>
                      </>
                    )}
                  </CCol>
                  <CCol md={6}>
                    <h6>Scaling Information</h6>
                    {stream.stream_mode_details?.stream_mode === 'ON_DEMAND' ? (
                      <p>On-demand streams automatically scale based on traffic patterns.</p>
                    ) : (
                      <>
                        <p><strong>Write Capacity:</strong> 1,000 records/sec per shard</p>
                        <p><strong>Write Throughput:</strong> 1 MB/sec per shard</p>
                        <p><strong>Read Capacity:</strong> 2 MB/sec per shard</p>
                        <p><strong>Read Throughput:</strong> 5 transactions/sec per shard</p>
                      </>
                    )}
                  </CCol>
                </CRow>
              </CForm>
            </CTabPane>

            {/* Tags Tab */}
            <CTabPane visible={activeTab === 'tags'}>
              <div className="d-flex justify-content-between align-items-center mb-3">
                <h6>Stream Tags</h6>
                <CButton
                  color="primary"
                  size="sm"
                  onClick={() => setShowAddTagModal(true)}
                >
                  Add Tag
                </CButton>
              </div>

              {tags.length === 0 ? (
                <div className="text-center text-muted py-4">
                  No tags configured. Add tags to organize and manage your streams.
                </div>
              ) : (
                <CTable responsive>
                  <CTableHead>
                    <CTableRow>
                      <CTableHeaderCell>Key</CTableHeaderCell>
                      <CTableHeaderCell>Value</CTableHeaderCell>
                      <CTableHeaderCell>Actions</CTableHeaderCell>
                    </CTableRow>
                  </CTableHead>
                  <CTableBody>
                    {tags.map((tag, index) => (
                      <CTableRow key={index}>
                        <CTableDataCell>{tag.key}</CTableDataCell>
                        <CTableDataCell>{tag.value}</CTableDataCell>
                        <CTableDataCell>
                          <CButton
                            color="danger"
                            size="sm"
                            onClick={() => removeTag(tag.key)}
                          >
                            Remove
                          </CButton>
                        </CTableDataCell>
                      </CTableRow>
                    ))}
                  </CTableBody>
                </CTable>
              )}
            </CTabPane>
          </CTabContent>
        </CCardBody>
      </CCard>

      {/* Add Tag Modal */}
      <CModal
        visible={showAddTagModal}
        onClose={() => setShowAddTagModal(false)}
      >
        <CModalHeader>
          <CModalTitle>Add Tag</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <CForm>
            <CFormInput
              label="Tag Key"
              value={newTag.key}
              onChange={(e) => setNewTag({...newTag, key: e.target.value})}
              placeholder="Environment"
              className="mb-3"
            />
            <CFormInput
              label="Tag Value"
              value={newTag.value}
              onChange={(e) => setNewTag({...newTag, value: e.target.value})}
              placeholder="Production"
            />
          </CForm>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowAddTagModal(false)}>
            Cancel
          </CButton>
          <CButton color="primary" onClick={addTag}>
            Add Tag
          </CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default StreamConfiguration;
