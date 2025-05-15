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
