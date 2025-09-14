import React, { useState, useEffect, useRef } from 'react';
import {
  CCard,
  CCardHeader,
  CCardBody,
  CRow,
  CCol,
  CButton,
  CAlert,
  CSpinner,
  CBadge,
  CTable,
  CTableHead,
  CTableRow,
  CTableHeaderCell,
  CTableBody,
  CTableDataCell,
  CProgress,
  CModal,
  CModalHeader,
  CModalTitle,
  CModalBody,
  CModalFooter
} from '@coreui/react';
import { FaEye, FaRefresh, FaInfoCircle, FaChartBar } from 'react-icons/fa';
import KinesisService from '../../services/kinesisService';

const ShardVisualization = ({ profile, region, streamName, shards }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [selectedShard, setSelectedShard] = useState(null);
  const [showShardModal, setShowShardModal] = useState(false);
  const [shardMetrics, setShardMetrics] = useState({});
  const canvasRef = useRef(null);

  // Load shard metrics
  const loadShardMetrics = async () => {
    setLoading(true);
    setError(null);

    try {
      // Simulate shard metrics - in a real implementation, you'd get these from CloudWatch
      const metrics = {};
      for (const shard of shards) {
        metrics[shard.shard_id] = {
          incomingRecords: Math.floor(Math.random() * 1000),
          incomingBytes: Math.floor(Math.random() * 100000),
          outgoingRecords: Math.floor(Math.random() * 800),
          outgoingBytes: Math.floor(Math.random() * 80000),
          iteratorAge: Math.floor(Math.random() * 300000), // milliseconds
          writeThrottled: Math.floor(Math.random() * 10),
          readThrottled: Math.floor(Math.random() * 5)
        };
      }
      setShardMetrics(metrics);
    } catch (err) {
      setError(`Failed to load shard metrics: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Draw shard topology
  const drawShardTopology = () => {
    const canvas = canvasRef.current;
    if (!canvas || shards.length === 0) return;

    const ctx = canvas.getContext('2d');
    const width = canvas.width;
    const height = canvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Calculate shard positions
    const shardWidth = 120;
    const shardHeight = 60;
    const padding = 20;
    const cols = Math.ceil(Math.sqrt(shards.length));
    const rows = Math.ceil(shards.length / cols);

    const startX = (width - (cols * (shardWidth + padding) - padding)) / 2;
    const startY = (height - (rows * (shardHeight + padding) - padding)) / 2;

    shards.forEach((shard, index) => {
      const col = index % cols;
      const row = Math.floor(index / cols);
      const x = startX + col * (shardWidth + padding);
      const y = startY + row * (shardHeight + padding);

      // Determine shard color based on status
      let color = '#28a745'; // Active - green
      if (shard.sequence_number_range.ending_sequence_number) {
        color = '#6c757d'; // Closed - gray
      }

      // Draw shard rectangle
      ctx.fillStyle = color;
      ctx.fillRect(x, y, shardWidth, shardHeight);

      // Draw border
      ctx.strokeStyle = '#000';
      ctx.lineWidth = 2;
      ctx.strokeRect(x, y, shardWidth, shardHeight);

      // Draw shard ID
      ctx.fillStyle = '#fff';
      ctx.font = '12px Arial';
      ctx.textAlign = 'center';
      ctx.fillText(
        shard.shard_id.substring(shard.shard_id.lastIndexOf('-') + 1),
        x + shardWidth / 2,
        y + shardHeight / 2 - 5
      );

      // Draw metrics if available
      const metrics = shardMetrics[shard.shard_id];
      if (metrics) {
        ctx.font = '10px Arial';
        ctx.fillText(
          `In: ${metrics.incomingRecords}`,
          x + shardWidth / 2,
          y + shardHeight / 2 + 8
        );
        ctx.fillText(
          `Out: ${metrics.outgoingRecords}`,
          x + shardWidth / 2,
          y + shardHeight / 2 + 18
        );
      }

      // Draw parent relationships
      if (shard.parent_shard_id) {
        const parentIndex = shards.findIndex(s => s.shard_id === shard.parent_shard_id);
        if (parentIndex !== -1) {
          const parentCol = parentIndex % cols;
          const parentRow = Math.floor(parentIndex / cols);
          const parentX = startX + parentCol * (shardWidth + padding) + shardWidth / 2;
          const parentY = startY + parentRow * (shardHeight + padding) + shardHeight;
          
          const childX = x + shardWidth / 2;
          const childY = y;

          // Draw arrow from parent to child
          ctx.beginPath();
          ctx.moveTo(parentX, parentY);
          ctx.lineTo(childX, childY);
          ctx.strokeStyle = '#007bff';
          ctx.lineWidth = 2;
          ctx.stroke();

          // Draw arrowhead
          const angle = Math.atan2(childY - parentY, childX - parentX);
          const arrowLength = 10;
          ctx.beginPath();
          ctx.moveTo(childX, childY);
          ctx.lineTo(
            childX - arrowLength * Math.cos(angle - Math.PI / 6),
            childY - arrowLength * Math.sin(angle - Math.PI / 6)
          );
          ctx.moveTo(childX, childY);
          ctx.lineTo(
            childX - arrowLength * Math.cos(angle + Math.PI / 6),
            childY - arrowLength * Math.sin(angle + Math.PI / 6)
          );
          ctx.stroke();
        }
      }
    });

    // Add legend
    const legendY = height - 80;
    ctx.fillStyle = '#28a745';
    ctx.fillRect(20, legendY, 20, 15);
    ctx.fillStyle = '#000';
    ctx.font = '12px Arial';
    ctx.textAlign = 'left';
    ctx.fillText('Active Shard', 50, legendY + 12);

    ctx.fillStyle = '#6c757d';
    ctx.fillRect(150, legendY, 20, 15);
    ctx.fillText('Closed Shard', 180, legendY + 12);

    ctx.strokeStyle = '#007bff';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(280, legendY + 7);
    ctx.lineTo(320, legendY + 7);
    ctx.stroke();
    ctx.fillText('Parent-Child Relationship', 330, legendY + 12);
  };

  // Handle canvas click
  const handleCanvasClick = (event) => {
    const canvas = canvasRef.current;
    const rect = canvas.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;

    // Find clicked shard
    const shardWidth = 120;
    const shardHeight = 60;
    const padding = 20;
    const cols = Math.ceil(Math.sqrt(shards.length));
    const rows = Math.ceil(shards.length / cols);

    const startX = (canvas.width - (cols * (shardWidth + padding) - padding)) / 2;
    const startY = (canvas.height - (rows * (shardHeight + padding) - padding)) / 2;

    for (let index = 0; index < shards.length; index++) {
      const col = index % cols;
      const row = Math.floor(index / cols);
      const shardX = startX + col * (shardWidth + padding);
      const shardY = startY + row * (shardHeight + padding);

      if (x >= shardX && x <= shardX + shardWidth && 
          y >= shardY && y <= shardY + shardHeight) {
        setSelectedShard(shards[index]);
        setShowShardModal(true);
        break;
      }
    }
  };

  // Get shard status
  const getShardStatus = (shard) => {
    if (shard.sequence_number_range.ending_sequence_number) {
      return { status: 'CLOSED', color: 'secondary' };
    }
    return { status: 'ACTIVE', color: 'success' };
  };

  // Calculate hash key range utilization
  const calculateHashKeyUtilization = (shard) => {
    const startHash = BigInt(shard.hash_key_range.starting_hash_key);
    const endHash = BigInt(shard.hash_key_range.ending_hash_key);
    const maxHash = BigInt('340282366920938463463374607431768211455');
    
    const range = endHash - startHash;
    const utilization = Number((range * BigInt(100)) / maxHash);
    return Math.round(utilization * 100) / 100;
  };

  useEffect(() => {
    loadShardMetrics();
  }, [shards]);

  useEffect(() => {
    if (shards.length > 0) {
      drawShardTopology();
    }
  }, [shards, shardMetrics]);

  return (
    <div>
      {/* Error Alert */}
      {error && (
        <CAlert color="danger">
          {error}
        </CAlert>
      )}

      {/* Controls */}
      <CRow className="mb-3">
        <CCol>
          <CButton
            color="primary"
            onClick={loadShardMetrics}
            disabled={loading}
          >
            {loading ? <CSpinner size="sm" /> : <FaRefresh />}
            {loading ? ' Loading Metrics...' : ' Refresh Metrics'}
          </CButton>
        </CCol>
      </CRow>

      <CRow>
        {/* Shard Topology Visualization */}
        <CCol md={8}>
          <CCard>
            <CCardHeader>
              <FaEye className="me-2" />
              Shard Topology - {streamName}
            </CCardHeader>
            <CCardBody>
              <canvas
                ref={canvasRef}
                width={600}
                height={400}
                style={{ border: '1px solid #ccc', cursor: 'pointer' }}
                onClick={handleCanvasClick}
              />
              <div className="mt-2 text-muted">
                <small>Click on a shard to view details</small>
              </div>
            </CCardBody>
          </CCard>
        </CCol>

        {/* Shard Summary */}
        <CCol md={4}>
          <CCard>
            <CCardHeader>Shard Summary</CCardHeader>
            <CCardBody>
              <div className="mb-3">
                <strong>Total Shards:</strong> {shards.length}
              </div>
              <div className="mb-3">
                <strong>Active Shards:</strong>{' '}
                {shards.filter(s => !s.sequence_number_range.ending_sequence_number).length}
              </div>
              <div className="mb-3">
                <strong>Closed Shards:</strong>{' '}
                {shards.filter(s => s.sequence_number_range.ending_sequence_number).length}
              </div>
              
              {/* Shard Health Indicators */}
              <div className="mt-4">
                <h6>Shard Health</h6>
                {Object.entries(shardMetrics).map(([shardId, metrics]) => (
                  <div key={shardId} className="mb-2">
                    <div className="d-flex justify-content-between">
                      <small>{shardId.substring(shardId.lastIndexOf('-') + 1)}</small>
                      <CBadge color={metrics.iteratorAge > 60000 ? 'warning' : 'success'}>
                        {metrics.iteratorAge > 60000 ? 'Behind' : 'Current'}
                      </CBadge>
                    </div>
                    <CProgress
                      value={Math.min((metrics.iteratorAge / 300000) * 100, 100)}
                      color={metrics.iteratorAge > 60000 ? 'warning' : 'success'}
                      size="xs"
                    />
                  </div>
                ))}
              </div>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      {/* Detailed Shard Table */}
      <CCard className="mt-3">
        <CCardHeader>
          <FaChartBar className="me-2" />
          Shard Details
        </CCardHeader>
        <CCardBody>
          <CTable responsive hover>
            <CTableHead>
              <CTableRow>
                <CTableHeaderCell>Shard ID</CTableHeaderCell>
                <CTableHeaderCell>Status</CTableHeaderCell>
                <CTableHeaderCell>Hash Key Range</CTableHeaderCell>
                <CTableHeaderCell>Sequence Range</CTableHeaderCell>
                <CTableHeaderCell>Parent</CTableHeaderCell>
                <CTableHeaderCell>Metrics</CTableHeaderCell>
                <CTableHeaderCell>Actions</CTableHeaderCell>
              </CTableRow>
            </CTableHead>
            <CTableBody>
              {shards.map((shard) => {
                const status = getShardStatus(shard);
                const metrics = shardMetrics[shard.shard_id];
                const utilization = calculateHashKeyUtilization(shard);

                return (
                  <CTableRow key={shard.shard_id}>
                    <CTableDataCell>
                      <small>{shard.shard_id}</small>
                    </CTableDataCell>
                    <CTableDataCell>
                      <CBadge color={status.color}>{status.status}</CBadge>
                    </CTableDataCell>
                    <CTableDataCell>
                      <small>
                        {shard.hash_key_range.starting_hash_key} - {shard.hash_key_range.ending_hash_key}
                        <br />
                        <CBadge color="info" size="sm">{utilization}% of keyspace</CBadge>
                      </small>
                    </CTableDataCell>
                    <CTableDataCell>
                      <small>
                        Start: {shard.sequence_number_range.starting_sequence_number}
                        <br />
                        {shard.sequence_number_range.ending_sequence_number ? (
                          `End: ${shard.sequence_number_range.ending_sequence_number}`
                        ) : (
                          <CBadge color="success" size="sm">Open</CBadge>
                        )}
                      </small>
                    </CTableDataCell>
                    <CTableDataCell>
                      {shard.parent_shard_id ? (
                        <small>{shard.parent_shard_id}</small>
                      ) : (
                        <CBadge color="secondary" size="sm">Root</CBadge>
                      )}
                    </CTableDataCell>
                    <CTableDataCell>
                      {metrics ? (
                        <small>
                          In: {metrics.incomingRecords} rec/s<br />
                          Out: {metrics.outgoingRecords} rec/s<br />
                          Age: {Math.round(metrics.iteratorAge / 1000)}s
                        </small>
                      ) : (
                        <CSpinner size="sm" />
                      )}
                    </CTableDataCell>
                    <CTableDataCell>
                      <CButton
                        color="info"
                        size="sm"
                        onClick={() => {
                          setSelectedShard(shard);
                          setShowShardModal(true);
                        }}
                      >
                        <FaInfoCircle />
                      </CButton>
                    </CTableDataCell>
                  </CTableRow>
                );
              })}
            </CTableBody>
          </CTable>
        </CCardBody>
      </CCard>

      {/* Shard Detail Modal */}
      <CModal
        visible={showShardModal}
        onClose={() => setShowShardModal(false)}
        size="lg"
      >
        <CModalHeader>
          <CModalTitle>Shard Details</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {selectedShard && (
            <>
              <CRow>
                <CCol md={6}>
                  <h6>Basic Information</h6>
                  <p><strong>Shard ID:</strong> {selectedShard.shard_id}</p>
                  <p><strong>Status:</strong> {getShardStatus(selectedShard).status}</p>
                  <p><strong>Parent Shard:</strong> {selectedShard.parent_shard_id || 'None'}</p>
                </CCol>
                <CCol md={6}>
                  <h6>Hash Key Range</h6>
                  <p><strong>Starting:</strong> {selectedShard.hash_key_range.starting_hash_key}</p>
                  <p><strong>Ending:</strong> {selectedShard.hash_key_range.ending_hash_key}</p>
                  <p><strong>Utilization:</strong> {calculateHashKeyUtilization(selectedShard)}% of keyspace</p>
                </CCol>
              </CRow>
              
              <CRow className="mt-3">
                <CCol md={6}>
                  <h6>Sequence Number Range</h6>
                  <p><strong>Starting:</strong> {selectedShard.sequence_number_range.starting_sequence_number}</p>
                  <p><strong>Ending:</strong> {selectedShard.sequence_number_range.ending_sequence_number || 'Open'}</p>
                </CCol>
                <CCol md={6}>
                  <h6>Metrics</h6>
                  {shardMetrics[selectedShard.shard_id] && (
                    <>
                      <p><strong>Incoming Records:</strong> {shardMetrics[selectedShard.shard_id].incomingRecords}/s</p>
                      <p><strong>Outgoing Records:</strong> {shardMetrics[selectedShard.shard_id].outgoingRecords}/s</p>
                      <p><strong>Iterator Age:</strong> {Math.round(shardMetrics[selectedShard.shard_id].iteratorAge / 1000)}s</p>
                      <p><strong>Write Throttled:</strong> {shardMetrics[selectedShard.shard_id].writeThrottled}</p>
                      <p><strong>Read Throttled:</strong> {shardMetrics[selectedShard.shard_id].readThrottled}</p>
                    </>
                  )}
                </CCol>
              </CRow>
            </>
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setShowShardModal(false)}>
            Close
          </CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default ShardVisualization;
