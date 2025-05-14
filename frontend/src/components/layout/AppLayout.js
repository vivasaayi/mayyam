import React from "react";
import {
  CContainer,
  CHeader,
  CHeaderBrand,
  CHeaderNav,
  CNavItem,
  CNavLink,
  CSidebar,
  CSidebarBrand,
  CSidebarNav,
  CNavTitle,
  CSidebarToggler
} from "@coreui/react";
import { useNavigate } from "react-router-dom";
import { useAuth } from "../../hooks/useAuth";

const AppLayout = ({ children }) => {
  const [sidebarVisible, setSidebarVisible] = React.useState(true);
  const { logout, user } = useAuth();
  const navigate = useNavigate();

  const handleLogout = () => {
    logout();
    navigate("/login");
  };

  return (
    <div className="app-container">
      <CSidebar
        visible={sidebarVisible}
        onVisibleChange={(visible) => setSidebarVisible(visible)}
      >
        <CSidebarBrand>Mayyam</CSidebarBrand>
        <CSidebarNav>
          <CNavTitle>Features</CNavTitle>
          <CNavItem href="/dashboard">
            <CNavLink to="/dashboard">Dashboard</CNavLink>
          </CNavItem>
          <CNavItem href="/databases">
            <CNavLink to="/databases">Databases</CNavLink>
          </CNavItem>
          <CNavItem href="/kafka">
            <CNavLink to="/kafka">Kafka</CNavLink>
          </CNavItem>
          <CNavItem href="/cloud">
            <CNavLink to="/cloud">Cloud</CNavLink>
          </CNavItem>
          <CNavItem href="/kubernetes">
            <CNavLink to="/kubernetes">Kubernetes</CNavLink>
          </CNavItem>
          <CNavItem href="/chaos">
            <CNavLink to="/chaos">Chaos Engineering</CNavLink>
          </CNavItem>

          <CNavTitle>Settings</CNavTitle>
          <CNavItem href="/settings">
            <CNavLink to="/settings">Settings</CNavLink>
          </CNavItem>
        </CSidebarNav>
        <CSidebarToggler 
          onClick={() => setSidebarVisible(!sidebarVisible)} 
        />
      </CSidebar>

      <div className="wrapper d-flex flex-column min-vh-100">
        <CHeader className="header">
          <CContainer fluid className="d-flex align-items-center justify-content-between">
            <CHeaderBrand>
              <div className="d-flex align-items-center">
                <CSidebarToggler 
                  onClick={() => setSidebarVisible(!sidebarVisible)}
                  className="d-md-none"
                />
                <h4 className="mb-0 ms-2">Mayyam</h4>
              </div>
            </CHeaderBrand>
            <CHeaderNav className="ms-auto">
              {user && (
                <CNavItem>
                  <CNavLink onClick={handleLogout} style={{ cursor: "pointer" }}>
                    Logout
                  </CNavLink>
                </CNavItem>
              )}
            </CHeaderNav>
          </CContainer>
        </CHeader>

        <div className="body flex-grow-1 px-3 py-3">
          <CContainer fluid>{children}</CContainer>
        </div>
      </div>
    </div>
  );
};

export default AppLayout;
