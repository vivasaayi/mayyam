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


import React from 'react';
import {
  CCard,
  CCardBody,
  CCardHeader,
  CNav,
  CNavItem,
  CNavLink,
  CTabContent,
  CTabPane
} from '@coreui/react';

/**
 * SharedResourcesNav component
 * 
 * A reusable component for navigation between different types of shared resources
 * Used in the layout of pages that manage resources common to all clusters
 */
const SharedResourcesNav = ({ activeResource, onResourceChange, children }) => {
  return (
    <>
      <CCard className="mb-4">
        <CCardHeader>
          <h4>Shared Resources Management</h4>
          <p className="text-medium-emphasis">
            Manage resources that are shared across all clusters in your environment
          </p>
        </CCardHeader>
        <CCardBody>
          <CNav variant="tabs">
            <CNavItem>
              <CNavLink
                active={activeResource === 'queryTemplates'}
                onClick={() => onResourceChange('queryTemplates')}
                href="#/query-templates"
              >
                Query Templates
              </CNavLink>
            </CNavItem>
            {/* Add more shared resource types here as needed */}
            <CNavItem>
              <CNavLink
                active={activeResource === 'promptTemplates'}
                onClick={() => onResourceChange('promptTemplates')}
                href="#/prompt-templates"
              >
                Prompt Templates
              </CNavLink>
            </CNavItem>
            <CNavItem>
              <CNavLink
                active={activeResource === 'configurations'}
                onClick={() => onResourceChange('configurations')}
                href="#/configurations"
              >
                Configurations
              </CNavLink>
            </CNavItem>
          </CNav>
          
          <CTabContent className="mt-3">
            <CTabPane visible={true}>
              {children}
            </CTabPane>
          </CTabContent>
        </CCardBody>
      </CCard>
    </>
  );
};

export default SharedResourcesNav;
