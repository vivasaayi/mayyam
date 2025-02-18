import React, { useState } from 'react';
import { ComposableMap, Geographies, Geography, Marker, Line } from 'react-simple-maps';
import { Menu, Item, contextMenu } from 'react-contexify';
import 'react-contexify/dist/ReactContexify.css';
import { CModal, CModalHeader, CModalTitle, CModalBody, CModalFooter, CButton } from '@coreui/react';

const geoUrl = "https://unpkg.com/world-atlas@1/world/110m.json";

const ClusterMapVisualization = ({ cluster, onRegionClick }) => {
  console.log('ClusterMapVisualization received cluster:', cluster.globalClusterId);
  const { globalClusterId, replicationFlows, coordinates, region, status } = cluster;
  const [confirmationVisible, setConfirmationVisible] = useState(false);
  const [selectedFlow, setSelectedFlow] = useState(null);

  const handleMarkerClick = (region) => {
    onRegionClick(region);
  };

  const handleContextMenu = (event, region, id) => {
    event.preventDefault();
    contextMenu.show({
      id: id,
      event: event,
    });
  };

  const handleMouseEnter = (event, region, id) => {
    contextMenu.show({
      id: id,
      event: event,
    });
  };

  const handleMakePrimary = async (targetRegion, targetDbClusterIdentifier) => {
    setSelectedFlow({ targetRegion, targetDbClusterIdentifier });
    setConfirmationVisible(true);
  };

  const confirmMakePrimary = async () => {
    const { targetRegion, targetDbClusterIdentifier } = selectedFlow;
    setConfirmationVisible(false);
    try {
      await fetch(`/api/rds/failover?region=${region}&clusterId=${globalClusterId}&targetRegion=${targetRegion}&targetDbClusterIdentifier=${targetDbClusterIdentifier}`, {
        method: 'POST',
      });
      alert(`Failover initiated to ${targetRegion}`);
    } catch (error) {
      console.error("Failed to initiate failover:", error);
      alert("Failed to initiate failover");
    }
  };

  const getClusterNameFromArn = (arn) => {
    const parts = arn.split(':');
    return parts[parts.length - 1];
  };

  console.log('Cluster coordinates:', coordinates);
  replicationFlows.forEach((flow, index) => {
    console.log(`Replication flow ${index}:`, flow);
    console.log(`Source coordinates: ${coordinates}`);
    console.log(`Target coordinates: ${flow.targetCoordinates}`);
  });

  return (
    <div>
      <ComposableMap
        projection="geoMercator"
        projectionConfig={{
          scale: 100,   
          center: [0, 20], 
        }}
        style={{ width: "100%", height: "auto" }}>
        <Geographies geography={geoUrl}>
          {({ geographies }) =>
            geographies.map(geo => (
              <Geography key={geo.rsmKey} geography={geo} style={{
                default: { fill: "#D6D6DA" },
                hover: { fill: "#F53" },
                pressed: { fill: "#E42" }
              }} />
            ))
          }
        </Geographies>
        <Marker coordinates={coordinates} onClick={() => handleMarkerClick(region)} onMouseEnter={(e) => handleMouseEnter(e, region, `menu-${globalClusterId}`)}>
          <circle r={5} fill="green" />
          <text textAnchor="middle" y={-10} style={{ fontFamily: "system-ui", fill: "#5D5A6D" }}>
            {globalClusterId}
          </text>
        </Marker>
        {replicationFlows.map((flow, index) => (
          <React.Fragment key={index}>
            <Marker coordinates={flow.targetCoordinates} onClick={() => handleMarkerClick(flow.targetRegion)} onMouseEnter={(e) => handleMouseEnter(e, flow.targetRegion, `menu-${globalClusterId}-${index}`)}>
              <circle r={5} fill="red" />
              <text textAnchor="middle" y={-10} style={{ fontFamily: "system-ui", fill: "#5D5A6D" }}>
                {getClusterNameFromArn(flow.targetArn)}
              </text>
            </Marker>
            <Line
              from={coordinates}
              to={flow.targetCoordinates}
              stroke={status === "switching-over" ? "orange" : "#00f"}
              strokeWidth={2}
              strokeLinecap="round"
              style={status === "switching-over" ? { animation: "dash 2s linear infinite" } : {}}
            />
          </React.Fragment>
        ))}
      </ComposableMap>
      {replicationFlows.map((flow, index) => (
        <Menu id={`menu-${globalClusterId}-${index}`} key={index}>
          <Item onClick={() => handleMakePrimary(flow.targetRegion, flow.targetArn)}>Make {getClusterNameFromArn(flow.targetArn)} as Primary</Item>
        </Menu>
      ))}
      <CModal visible={confirmationVisible} onClose={() => setConfirmationVisible(false)}>
        <CModalHeader closeButton>
          <CModalTitle>Confirm Failover</CModalTitle>
        </CModalHeader>
        <CModalBody>
          <p>Are you sure you want to initiate failover to the selected region?</p>
          <p><strong>Cluster ID:</strong> {globalClusterId}</p>
          <p><strong>Target Region:</strong> {selectedFlow?.targetRegion}</p>
          <p><strong>Target DB Cluster Identifier:</strong> {selectedFlow?.targetDbClusterIdentifier}</p>
        </CModalBody>
        <CModalFooter>
          <CButton color="secondary" onClick={() => setConfirmationVisible(false)}>Cancel</CButton>
          <CButton color="primary" onClick={confirmMakePrimary}>Confirm</CButton>
        </CModalFooter>
      </CModal>
      <style>
        {`
          @keyframes dash {
            to {
              stroke-dashoffset: 1000;
            }
          }
          line[style*="animation: dash"] {
            stroke-dasharray: 5, 5;
            stroke-dashoffset: 0;
          }
        `}
      </style>
    </div>
  );
};

export default ClusterMapVisualization;
