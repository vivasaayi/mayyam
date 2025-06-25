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
    <div className="d-flex" style={{ minHeight: "100vh" }}>
      <CSidebar
        className="d-print-none sidebar sidebar-fixed"
        visible={sidebarVisible}
        onVisibleChange={(visible) => setSidebarVisible(visible)}
        position="fixed"
        colorScheme="dark"
      >
        <CSidebarBrand className="d-md-down-none">
          <div className="text-decoration-none text-white fw-bold">
            Mayyam
          </div>
        </CSidebarBrand>
        <CSidebarNav>
          <CNavTitle>Features</CNavTitle>
          <CNavItem>
            <CNavLink as={Link} to="/">Dashboard</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/databases">Databases</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/kafka">Kafka</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/cloud">Cloud</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/kubernetes">Kubernetes</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/chaos">Chaos Engineering</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/csv-comparer">CSV Comparer</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/chat">AI Chat</CNavLink>
          </CNavItem>

          <CNavTitle>Settings</CNavTitle>
          <CNavItem>
            <CNavLink as={Link} to="/query-templates">Query Templates</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/llm-providers">LLM Providers</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/manage-kubernetes-clusters">Manage Clusters</CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink as={Link} to="/debug">Debug</CNavLink>
          </CNavItem>
        </CSidebarNav>
        <CSidebarToggler 
          className="d-none d-lg-flex"
          onClick={() => setSidebarVisible(!sidebarVisible)} 
        />
      </CSidebar>

      <div className="wrapper d-flex flex-column min-vh-100" style={{ marginLeft: sidebarVisible ? '256px' : '0', width: '100%', transition: 'margin-left 0.15s' }}>
        <CHeader position="sticky" className="mb-4">
          <CContainer fluid className="d-flex align-items-center">
            <CHeaderBrand className="me-auto">
              <CSidebarToggler 
                className="ps-1"
                onClick={() => setSidebarVisible(!sidebarVisible)}
                style={{ marginInlineStart: '-14px' }}
              />
              <span className="ms-2 fs-4">Mayyam</span>
            </CHeaderBrand>
            <CHeaderNav>
              {user && (
                <CNavItem>
                  <CNavLink 
                    href="#" 
                    onClick={(e) => {
                      e.preventDefault();
                      handleLogout();
                    }}
                    style={{ cursor: "pointer" }}
                  >
                    Logout ({user.username})
                  </CNavLink>
                </CNavItem>
              )}
            </CHeaderNav>
          </CContainer>
        </CHeader>

        <div className="body flex-grow-1">
          <CContainer fluid className="h-auto px-4">
            <Outlet />
          </CContainer>
        </div>
      </div>
    </div>
  );
};

export default AppLayout;
