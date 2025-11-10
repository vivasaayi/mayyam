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


import React from "react";
import { CBreadcrumb, CBreadcrumbItem } from "@coreui/react";
import { Link } from "react-router-dom";

const PageHeader = ({ title, breadcrumbs = [] }) => {
  return (
    <div className="page-header mb-4">
      <div className="d-flex justify-content-between align-items-center">
        <h1 className="mb-0">{title}</h1>
      </div>
      
      {breadcrumbs.length > 0 && (
        <CBreadcrumb className="mt-2">
          <CBreadcrumbItem>
            <Link to="/">Home</Link>
          </CBreadcrumbItem>
          
          {breadcrumbs.map((breadcrumb, index) => (
            <CBreadcrumbItem 
              key={index} 
              active={index === breadcrumbs.length - 1}
            >
              {breadcrumb.link && index !== breadcrumbs.length - 1 ? (
                <Link to={breadcrumb.link}>{breadcrumb.label}</Link>
              ) : (
                breadcrumb.label
              )}
            </CBreadcrumbItem>
          ))}
        </CBreadcrumb>
      )}
    </div>
  );
};

export default PageHeader;
