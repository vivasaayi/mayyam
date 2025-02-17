import React, { useState } from 'react';
import { CNav, CNavItem, CNavLink, CTabContent, CTabPane, CButton } from '@coreui/react';
import TablesWithoutReplication from './TablesWithoutReplication';
import TablesWithReplication from './TablesWithReplication';

const DynamoDbReplication = () => {
  const [activeKey, setActiveKey] = useState(1);

  return (
    <div>
      <CNav variant="tabs">
        <CNavItem>
          <CNavLink
            active={activeKey === 1}
            onClick={() => setActiveKey(1)}
          >
            Regional Tables
          </CNavLink>
        </CNavItem>
        <CNavItem>
          <CNavLink
            active={activeKey === 2}
            onClick={() => setActiveKey(2)}
          >
            Global Tables
          </CNavLink>
        </CNavItem>
      </CNav>
      <CTabContent>
        <CTabPane visible={activeKey === 1}>
          <TablesWithoutReplication />
        </CTabPane>
        <CTabPane visible={activeKey === 2}>
          <TablesWithReplication />
        </CTabPane>
      </CTabContent>
    </div>
  );
};

export default DynamoDbReplication;
