import React, { useState } from 'react';
import {
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane,
  CCard,
  CCardBody,
} from '@coreui/react';
import CIcon from '@coreui/icons-react';
import { cilChartBar, cilClipboard, cilTask, cilFile } from '@coreui/icons';

// Import the sub-components
import Chaos from './Chaos';
import ChaosAuditLogs from './ChaosAuditLogs';
import ChaosMetricsDashboard from './ChaosMetricsDashboard';
import ComplianceReportGenerator from './ComplianceReportGenerator';

const ChaosEngineering = () => {
  const [activeTab, setActiveTab] = useState('experiments');

  return (
    <div>
      <CCard>
        <CCardBody>
          <CNav variant="tabs" role="tablist">
            <CNavItem role="presentation">
              <CNavLink
                active={activeTab === 'experiments'}
                component="button"
                role="tab"
                onClick={() => setActiveTab('experiments')}
              >
                <CIcon icon={cilTask} className="me-2" />
                Experiments
              </CNavLink>
            </CNavItem>
            <CNavItem role="presentation">
              <CNavLink
                active={activeTab === 'metrics'}
                component="button"
                role="tab"
                onClick={() => setActiveTab('metrics')}
              >
                <CIcon icon={cilChartBar} className="me-2" />
                Metrics
              </CNavLink>
            </CNavItem>
            <CNavItem role="presentation">
              <CNavLink
                active={activeTab === 'audit'}
                component="button"
                role="tab"
                onClick={() => setActiveTab('audit')}
              >
                <CIcon icon={cilClipboard} className="me-2" />
                Audit Logs
              </CNavLink>
            </CNavItem>
            <CNavItem role="presentation">
              <CNavLink
                active={activeTab === 'compliance'}
                component="button"
                role="tab"
                onClick={() => setActiveTab('compliance')}
              >
                <CIcon icon={cilFile} className="me-2" />
                Compliance Reports
              </CNavLink>
            </CNavItem>
          </CNav>

          <CTabContent className="mt-3">
            <CTabPane role="tabpanel" visible={activeTab === 'experiments'}>
              <Chaos />
            </CTabPane>
            <CTabPane role="tabpanel" visible={activeTab === 'metrics'}>
              <ChaosMetricsDashboard />
            </CTabPane>
            <CTabPane role="tabpanel" visible={activeTab === 'audit'}>
              <ChaosAuditLogs />
            </CTabPane>
            <CTabPane role="tabpanel" visible={activeTab === 'compliance'}>
              <ComplianceReportGenerator />
            </CTabPane>
          </CTabContent>
        </CCardBody>
      </CCard>
    </div>
  );
};

export default ChaosEngineering;
