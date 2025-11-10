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


import React, { useState, useEffect, useCallback } from 'react';
import {
  CContainer,
  CRow,
  CCol,
  CCard,
  CCardHeader,
  CCardBody,
  CButton,
  CForm,
  CFormInput,
  CFormSelect,
  CAlert,
  CBadge,
  CSpinner,
  CButtonGroup,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane,
  CProgress,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell
} from '@coreui/react';
import {
  FaStream,
  FaPlay,
  FaStop,
  FaPlus,
  FaTrash,
  FaSync,
  FaChartLine,
  FaCog,
  FaDatabase,
  FaEye
} from 'react-icons/fa';
import KinesisService from '../../services/kinesisService';
import StreamMetricsChart from './StreamMetricsChart';
import RecordOperations from './RecordOperations';
import ShardVisualization from './ShardVisualization';
import StreamConfiguration from './StreamConfiguration';

const KinesisDashboard = () => {
  const [activeTab, setActiveTab] = useState('streams');
  const [profile, setProfile] = useState('default');
  const [region, setRegion] = useState('us-east-1');
  const [streams, setStreams] = useState([]);
  const [selectedStream, setSelectedStream] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);
  const [limits, setLimits] = useState(null);
  const [refreshInterval, setRefreshInterval] = useState(null);

  // Available AWS regions
  const regions = [
    'us-east-1', 'us-west-2', 'eu-west-1', 'eu-central-1',
    'ap-southeast-1', 'ap-northeast-1', 'ca-central-1'
  ];

  // Clear messages
  const clearMessages = () => {
    setError(null);
    setSuccess(null);
  };

  // Load account limits
  const loadLimits = useCallback(async () => {
    try {
      const limitsData = await KinesisService.getStreamLimits(profile, region);
      setLimits(limitsData);
    } catch (err) {
      console.error('Failed to load limits:', err);
    }
  }, [profile, region]);

  // Load streams for the current profile/region
  const loadStreams = useCallback(async () => {
    setLoading(true);
    clearMessages();
    try {
      // Note: We'll need to implement a list streams endpoint or 
      // simulate it by tracking created streams
      const streamList = JSON.parse(localStorage.getItem(`kinesis_streams_${profile}_${region}`) || '[]');
      
      // Get detailed info for each stream
      const streamDetails = await Promise.allSettled(
        streamList.map(async (streamName) => {
          try {
            const details = await KinesisService.describeStream(profile, region, streamName);
            return { name: streamName, ...details };
          } catch (err) {
            return { name: streamName, status: 'ERROR', error: err.message };
          }
        })
      );

      setStreams(streamDetails
        .filter(result => result.status === 'fulfilled')
        .map(result => result.value)
      );
    } catch (err) {
      setError(`Failed to load streams: ${err.message}`);
    } finally {
      setLoading(false);
    }
  }, [profile, region]);

  // Auto-refresh streams
  useEffect(() => {
    loadStreams();
    loadLimits();
    
    // Set up auto-refresh every 30 seconds
    const interval = setInterval(loadStreams, 30000);
    setRefreshInterval(interval);
    
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [loadStreams, loadLimits]);

  // Handle profile/region change
  const handleProfileRegionChange = () => {
    setSelectedStream(null);
    loadStreams();
    loadLimits();
  };

  // Create new stream
  const handleCreateStream = async (streamData) => {
    setLoading(true);
    clearMessages();
    try {
      await KinesisService.createStream(
        profile,
        region,
        streamData.name,
        streamData.shardCount,
        streamData.streamModeDetails
      );
      
      // Add to local storage for tracking
      const existingStreams = JSON.parse(localStorage.getItem(`kinesis_streams_${profile}_${region}`) || '[]');
      if (!existingStreams.includes(streamData.name)) {
        existingStreams.push(streamData.name);
        localStorage.setItem(`kinesis_streams_${profile}_${region}`, JSON.stringify(existingStreams));
      }
      
      setSuccess(`Stream "${streamData.name}" created successfully`);
      setTimeout(loadStreams, 2000); // Reload after a delay for stream to be ready
    } catch (err) {
      setError(`Failed to create stream: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Delete stream
  const handleDeleteStream = async (streamName) => {
    if (!window.confirm(`Are you sure you want to delete stream "${streamName}"?`)) {
      return;
    }

    setLoading(true);
    clearMessages();
    try {
      await KinesisService.deleteStream(profile, region, streamName);
      
      // Remove from local storage
      const existingStreams = JSON.parse(localStorage.getItem(`kinesis_streams_${profile}_${region}`) || '[]');
      const updatedStreams = existingStreams.filter(name => name !== streamName);
      localStorage.setItem(`kinesis_streams_${profile}_${region}`, JSON.stringify(updatedStreams));
      
      setSuccess(`Stream "${streamName}" deleted successfully`);
      if (selectedStream?.name === streamName) {
        setSelectedStream(null);
      }
      loadStreams();
    } catch (err) {
      setError(`Failed to delete stream: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Stream status badge
  const getStatusBadge = (status) => {
    const statusColors = {
      'ACTIVE': 'success',
      'CREATING': 'warning',
      'DELETING': 'danger',
      'UPDATING': 'info',
      'ERROR': 'danger'
    };
    return <CBadge color={statusColors[status] || 'secondary'}>{status}</CBadge>;
  };

  return (
    <CContainer fluid>
      <CRow className="mb-4">
        <CCol>
          <h2><FaStream className="me-2" />Kinesis Management Console</h2>
        </CCol>
      </CRow>

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

      {/* Profile and Region Selection */}
      <CRow className="mb-4">
        <CCol md={6}>
          <CCard>
            <CCardBody>
              <CForm className="row g-3">
                <CCol md={6}>
                  <CFormInput
                    label="AWS Profile"
                    value={profile}
                    onChange={(e) => {
                      setProfile(e.target.value);
                      handleProfileRegionChange();
                    }}
                    placeholder="default"
                  />
                </CCol>
                <CCol md={6}>
                  <CFormSelect
                    label="AWS Region"
                    value={region}
                    onChange={(e) => {
                      setRegion(e.target.value);
                      handleProfileRegionChange();
                    }}
                  >
                    {regions.map(r => (
                      <option key={r} value={r}>{r}</option>
                    ))}
                  </CFormSelect>
                </CCol>
              </CForm>
            </CCardBody>
          </CCard>
        </CCol>
        
        {/* Account Limits */}
        {limits && (
          <CCol md={6}>
            <CCard>
              <CCardHeader>Account Limits</CCardHeader>
              <CCardBody>
                <CRow>
                  <CCol md={6}>
                    <strong>Shard Limit:</strong> {limits.shard_limit}
                  </CCol>
                  <CCol md={6}>
                    <strong>Open Shards:</strong> {limits.open_shard_count}
                  </CCol>
                </CRow>
              </CCardBody>
            </CCard>
          </CCol>
        )}
      </CRow>

      {/* Navigation Tabs */}
      <CRow>
        <CCol>
          <CCard>
            <CCardHeader>
              <CNav variant="tabs">
                <CNavItem>
                  <CNavLink
                    active={activeTab === 'streams'}
                    onClick={() => setActiveTab('streams')}
                    style={{ cursor: 'pointer' }}
                  >
                    <FaStream className="me-2" />Streams
                  </CNavLink>
                </CNavItem>
                <CNavItem>
                  <CNavLink
                    active={activeTab === 'metrics'}
                    onClick={() => setActiveTab('metrics')}
                    style={{ cursor: 'pointer' }}
                    disabled={!selectedStream}
                  >
                    <FaChartLine className="me-2" />Metrics
                  </CNavLink>
                </CNavItem>
                <CNavItem>
                  <CNavLink
                    active={activeTab === 'records'}
                    onClick={() => setActiveTab('records')}
                    style={{ cursor: 'pointer' }}
                    disabled={!selectedStream}
                  >
                    <FaDatabase className="me-2" />Records
                  </CNavLink>
                </CNavItem>
                <CNavItem>
                  <CNavLink
                    active={activeTab === 'shards'}
                    onClick={() => setActiveTab('shards')}
                    style={{ cursor: 'pointer' }}
                    disabled={!selectedStream}
                  >
                    <FaEye className="me-2" />Shards
                  </CNavLink>
                </CNavItem>
                <CNavItem>
                  <CNavLink
                    active={activeTab === 'config'}
                    onClick={() => setActiveTab('config')}
                    style={{ cursor: 'pointer' }}
                    disabled={!selectedStream}
                  >
                    <FaCog className="me-2" />Configuration
                  </CNavLink>
                </CNavItem>
              </CNav>
              
              <div className="d-flex justify-content-end">
                <CButton
                  color="primary"
                  size="sm"
                  onClick={loadStreams}
                  disabled={loading}
                >
                  {loading ? <CSpinner size="sm" /> : <FaSync />}
                  {loading ? ' Loading...' : ' Refresh'}
                </CButton>
              </div>
            </CCardHeader>

            <CCardBody>
              <CTabContent>
                {/* Streams Tab */}
                <CTabPane visible={activeTab === 'streams'}>
                  <StreamsList
                    streams={streams}
                    selectedStream={selectedStream}
                    onSelectStream={setSelectedStream}
                    onCreateStream={handleCreateStream}
                    onDeleteStream={handleDeleteStream}
                    loading={loading}
                  />
                </CTabPane>

                {/* Metrics Tab */}
                <CTabPane visible={activeTab === 'metrics'}>
                  {selectedStream && (
                    <StreamMetricsChart
                      profile={profile}
                      region={region}
                      streamName={selectedStream.name}
                    />
                  )}
                </CTabPane>

                {/* Records Tab */}
                <CTabPane visible={activeTab === 'records'}>
                  {selectedStream && (
                    <RecordOperations
                      profile={profile}
                      region={region}
                      streamName={selectedStream.name}
                      shards={selectedStream.shards || []}
                    />
                  )}
                </CTabPane>

                {/* Shards Tab */}
                <CTabPane visible={activeTab === 'shards'}>
                  {selectedStream && (
                    <ShardVisualization
                      profile={profile}
                      region={region}
                      streamName={selectedStream.name}
                      shards={selectedStream.shards || []}
                    />
                  )}
                </CTabPane>

                {/* Configuration Tab */}
                <CTabPane visible={activeTab === 'config'}>
                  {selectedStream && (
                    <StreamConfiguration
                      profile={profile}
                      region={region}
                      stream={selectedStream}
                      onUpdate={loadStreams}
                    />
                  )}
                </CTabPane>
              </CTabContent>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>
    </CContainer>
  );
};

// Streams List Component
const StreamsList = ({ streams, selectedStream, onSelectStream, onCreateStream, onDeleteStream, loading }) => {
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [newStream, setNewStream] = useState({
    name: '',
    shardCount: 1,
    streamModeDetails: { stream_mode: 'PROVISIONED' }
  });

  const handleSubmitCreate = (e) => {
    e.preventDefault();
    if (newStream.name.trim()) {
      onCreateStream(newStream);
      setNewStream({ name: '', shardCount: 1, streamModeDetails: { stream_mode: 'PROVISIONED' } });
      setShowCreateForm(false);
    }
  };

  return (
    <>
      {/* Create Stream Form */}
      {showCreateForm && (
        <CCard className="mb-3">
          <CCardHeader>Create New Stream</CCardHeader>
          <CCardBody>
            <CForm onSubmit={handleSubmitCreate}>
              <CRow>
                <CCol md={4}>
                  <CFormInput
                    label="Stream Name"
                    value={newStream.name}
                    onChange={(e) => setNewStream({...newStream, name: e.target.value})}
                    placeholder="my-kinesis-stream"
                    required
                  />
                </CCol>
                <CCol md={4}>
                  <CFormSelect
                    label="Stream Mode"
                    value={newStream.streamModeDetails.stream_mode}
                    onChange={(e) => setNewStream({
                      ...newStream,
                      streamModeDetails: { stream_mode: e.target.value }
                    })}
                  >
                    <option value="PROVISIONED">Provisioned</option>
                    <option value="ON_DEMAND">On-Demand</option>
                  </CFormSelect>
                </CCol>
                {newStream.streamModeDetails.stream_mode === 'PROVISIONED' && (
                  <CCol md={4}>
                    <CFormInput
                      label="Shard Count"
                      type="number"
                      min="1"
                      max="10000"
                      value={newStream.shardCount}
                      onChange={(e) => setNewStream({...newStream, shardCount: parseInt(e.target.value)})}
                    />
                  </CCol>
                )}
              </CRow>
              <CRow className="mt-3">
                <CCol>
                  <CButtonGroup>
                    <CButton type="submit" color="primary">
                      <FaPlus className="me-2" />Create Stream
                    </CButton>
                    <CButton 
                      type="button" 
                      color="secondary" 
                      onClick={() => setShowCreateForm(false)}
                    >
                      Cancel
                    </CButton>
                  </CButtonGroup>
                </CCol>
              </CRow>
            </CForm>
          </CCardBody>
        </CCard>
      )}

      {/* Create Button */}
      {!showCreateForm && (
        <div className="mb-3">
          <CButton 
            color="primary" 
            onClick={() => setShowCreateForm(true)}
            disabled={loading}
          >
            <FaPlus className="me-2" />Create Stream
          </CButton>
        </div>
      )}

      {/* Streams Table */}
      <CTable responsive hover>
        <CTableHead>
          <CTableRow>
            <CTableHeaderCell>Stream Name</CTableHeaderCell>
            <CTableHeaderCell>Status</CTableHeaderCell>
            <CTableHeaderCell>Shard Count</CTableHeaderCell>
            <CTableHeaderCell>Retention (hrs)</CTableHeaderCell>
            <CTableHeaderCell>Mode</CTableHeaderCell>
            <CTableHeaderCell>Actions</CTableHeaderCell>
          </CTableRow>
        </CTableHead>
        <CTableBody>
          {streams.length === 0 ? (
            <CTableRow>
              <CTableDataCell colSpan="6" className="text-center text-muted">
                {loading ? 'Loading streams...' : 'No streams found. Create your first stream!'}
              </CTableDataCell>
            </CTableRow>
          ) : (
            streams.map((stream) => (
              <CTableRow 
                key={stream.name}
                style={{ 
                  cursor: 'pointer',
                  backgroundColor: selectedStream?.name === stream.name ? '#f8f9fa' : 'transparent'
                }}
                onClick={() => onSelectStream(stream)}
              >
                <CTableDataCell>
                  <strong>{stream.name}</strong>
                  {selectedStream?.name === stream.name && (
                    <CBadge color="info" className="ms-2">Selected</CBadge>
                  )}
                </CTableDataCell>
                <CTableDataCell>
                  {getStatusBadge(stream.stream_status)}
                </CTableDataCell>
                <CTableDataCell>
                  {stream.shards ? stream.shards.length : 'N/A'}
                </CTableDataCell>
                <CTableDataCell>
                  {stream.retention_period_hours || 24}
                </CTableDataCell>
                <CTableDataCell>
                  <CBadge color="secondary">
                    {stream.stream_mode_details?.stream_mode || 'PROVISIONED'}
                  </CBadge>
                </CTableDataCell>
                <CTableDataCell>
                  <CButtonGroup>
                    <CButton
                      color="danger"
                      size="sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        onDeleteStream(stream.name);
                      }}
                      disabled={loading}
                    >
                      <FaTrash />
                    </CButton>
                  </CButtonGroup>
                </CTableDataCell>
              </CTableRow>
            ))
          )}
        </CTableBody>
      </CTable>
    </>
  );
};

// Helper function for status badge
const getStatusBadge = (status) => {
  const statusColors = {
    'ACTIVE': 'success',
    'CREATING': 'warning',
    'DELETING': 'danger',
    'UPDATING': 'info',
    'ERROR': 'danger'
  };
  return <CBadge color={statusColors[status] || 'secondary'}>{status}</CBadge>;
};

export default KinesisDashboard;
