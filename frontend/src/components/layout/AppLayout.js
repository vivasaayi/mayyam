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
import { Link } from "react-router-dom";
import { useNavigate } from "react-router-dom";
import { useAuth } from "../../hooks/useAuth";
import { Outlet } from "react-router-dom";

const AppLayout = () => {
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
          <CNavItem>
            <Link to="/dashboard" className="nav-link">Dashboard</Link>
          </CNavItem>
          <CNavItem>
            <Link to="/databases" className="nav-link">Databases</Link>
          </CNavItem>
          <CNavItem>
            <Link to="/kafka" className="nav-link">Kafka</Link>
          </CNavItem>
          <CNavItem>
            <Link to="/cloud" className="nav-link">Cloud</Link>
          </CNavItem>
          <CNavItem>
            <Link to="/kubernetes" className="nav-link">Kubernetes</Link>
          </CNavItem>
          <CNavItem>
            <Link to="/chaos" className="nav-link">Chaos Engineering</Link>
          </CNavItem>

          <CNavTitle>Settings</CNavTitle>
          <CNavItem>
            <Link to="/settings" className="nav-link">Settings</Link>
          </CNavItem>
          <CNavItem>
            <Link to="/debug" className="nav-link">Debug</Link>
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
                  <span onClick={handleLogout} className="nav-link" style={{ cursor: "pointer" }}>
                    Logout
                  </span>
                </CNavItem>
              )}
            </CHeaderNav>
          </CContainer>
        </CHeader>

        <div className="body flex-grow-1 px-3 py-3">
          <CContainer fluid>
            <Outlet />
          </CContainer>
        </div>
      </div>
    </div>
  );
};

export default AppLayout;
