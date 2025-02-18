import React, { useState, useEffect } from 'react';
import { CFormSelect } from '@coreui/react';

const RegionDropdown = ({ selectedRegion, onChange }) => {
  const [regions, setRegions] = useState([]);

  useEffect(() => {
    fetch('/api/aws/regions')
      .then(response => response.json())
      .then(data => setRegions(data))
      .catch(error => console.error('Error fetching regions:', error));
  }, []);

  return (
    <CFormSelect value={selectedRegion} onChange={onChange}>
      {regions.map(region => (
        <option key={region} value={region}>
          {region}
        </option>
      ))}
    </CFormSelect>
  );
};

export default RegionDropdown;