import React, { useState, useEffect } from 'react';
import { CModal, CModalHeader, CModalTitle, CModalBody, CModalFooter, CButton, CSpinner } from '@coreui/react';
import RegionDropdown from '../RegionDropdown';
import ClusterMapVisualization from './ClusterMapVisualization';

const ClusterMap = () => {
  const [clusters, setClusters] = useState([]);
  const [selectedRegion, setSelectedRegion] = useState('us-west-2');
  const [loading, setLoading] = useState(false);
  const [modalVisible, setModalVisible] = useState(false);
  const [regionDetails, setRegionDetails] = useState(null);

  useEffect(() => {
    fetchClusters();
  }, [selectedRegion]);

  const fetchClusters = async () => {
    setLoading(true);
    try {
      const response = await fetch(`/api/rds/global-clusters-with-replication-flows?region=${selectedRegion}`);
      const data = await response.json();
      setClusters(data);
      setLoading(false);
    } catch (error) {
      console.error("Failed to fetch clusters:", error);
      setLoading(false);
    }
  };

  const handleRegionClick = async (region) => {
    setSelectedRegion(region);
    setModalVisible(true);
    try {
      const response = await fetch(`/api/rds/region-details?region=${region}`);
      const data = await response.json();
      setRegionDetails(data);
    } catch (error) {
      console.error("Failed to fetch region details:", error);
    }
  };

  return (
    <div>
      <h2>Cluster Map</h2>
      <RegionDropdown selectedRegion={selectedRegion} onChange={(e) => setSelectedRegion(e.target.value)} />
      {loading ? (
        <CSpinner color="primary" />
      ) : (
        clusters.map(cluster => {
          console.log('Rendering ClusterMapVisualization for cluster:', cluster.globalClusterId);
          return (
            <div key={cluster.globalClusterId}>
              <h3>{cluster.globalClusterId}</h3>
              <ClusterMapVisualization cluster={cluster} onRegionClick={handleRegionClick} />
            </div>
          );
        })
      )}
      <CModal visible={modalVisible} onClose={() => setModalVisible(false)}>
        <CModalHeader closeButton>
          <CModalTitle>Region Details</CModalTitle>
        </CModalHeader>
        <CModalBody>
          {regionDetails ? (
            <pre>{JSON.stringify(regionDetails, null, 2)}</pre>
          ) : (
            <CSpinner color="primary" />
          )}
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setModalVisible(false)}>Close</CButton>
        </CModalFooter>
      </CModal>
    </div>
  );
};

export default ClusterMap;
