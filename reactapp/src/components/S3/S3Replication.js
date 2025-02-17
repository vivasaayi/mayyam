import React, { useState } from 'react';
import { CNav, CNavItem, CNavLink, CTabContent, CTabPane, CButton } from '@coreui/react';
import BucketsWithoutReplication from './BucketsWithoutReplication';
import BucketsWithReplication from './BucketsWithReplication';

const S3Replication = () => {
  const [activeKey, setActiveKey] = useState(1);

  return (
    <div>
      <CNav variant="tabs">
        <CNavItem>
          <CNavLink
            active={activeKey === 1}
            onClick={() => setActiveKey(1)}
          >
            Regional Buckets
          </CNavLink>
        </CNavItem>
        <CNavItem>
          <CNavLink
            active={activeKey === 2}
            onClick={() => setActiveKey(2)}
          >
            Replicated Buckets
          </CNavLink>
        </CNavItem>
      </CNav>
      <CTabContent>
        <CTabPane visible={activeKey === 1}>
          <BucketsWithoutReplication />
        </CTabPane>
        <CTabPane visible={activeKey === 2}>
          <BucketsWithReplication />
        </CTabPane>
      </CTabContent>
    </div>
  );
};

export default S3Replication;
