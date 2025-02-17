import React, { useEffect, useState } from 'react';
import { CCard, CCardBody, CCardHeader, CCol, CRow, CTable, CTableBody, CTableDataCell, CTableHead, CTableHeaderCell, CTableRow } from '@coreui/react';

const RdsClusters = () => {
  const [clusters, setClusters] = useState([]);

  useEffect(() => {
    fetch('/api/rds/clusters')
      .then(response => response.json())
      .then(data => setClusters(data))
      .catch(error => console.error('Error fetching RDS clusters:', error));
  }, []);

  return (
    <CRow>
      <CCol>
        <CCard>
          <CCardHeader>
            RDS Clusters
          </CCardHeader>
          <CCardBody>
            <CTable>
              <CTableHead>
                <CTableRow>
                  <CTableHeaderCell>Cluster Identifier</CTableHeaderCell>
                  <CTableHeaderCell>Status</CTableHeaderCell>
                  <CTableHeaderCell>Engine</CTableHeaderCell>
                </CTableRow>
              </CTableHead>
              <CTableBody>
                {clusters.map(cluster => (
                  <CTableRow key={cluster.dbClusterIdentifier}>
                    <CTableDataCell>{cluster.dbClusterIdentifier}</CTableDataCell>
                    <CTableDataCell>{cluster.status}</CTableDataCell>
                    <CTableDataCell>{cluster.engine}</CTableDataCell>
                  </CTableRow>
                ))}
              </CTableBody>
            </CTable>
          </CCardBody>
        </CCard>
      </CCol>
    </CRow>
  );
};

export default RdsClusters;
