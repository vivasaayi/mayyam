import React from 'react';
import { CButton } from '@coreui/react';

const KubernetesTriaging = () => {
  const handleLoadSearchDomain = () => {
    window.open('#/kubernetes/search-domain', '_blank');
  };

  return (
    <div>
      <CButton color="primary" onClick={handleLoadSearchDomain}>Load Search Domain</CButton>
    </div>
  );
};

export default KubernetesTriaging;
