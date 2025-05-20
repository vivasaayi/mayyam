import React, { Suspense, lazy } from "react";
import { Routes, Route, Navigate, Outlet } from "react-router-dom";
import { useAuth } from "./hooks/useAuth";

// Layout components
import AppLayout from "./components/layout/AppLayout";
import LoadingFallback from "./components/common/LoadingFallback";

// Lazy-loaded page components
const Login = lazy(() => import("./pages/Login"));
const Dashboard = lazy(() => import("./pages/Dashboard"));
const Databases = lazy(() => import("./pages/Databases"));
const Kafka = lazy(() => import("./pages/Kafka"));
const Cloud = lazy(() => import("./pages/Cloud"));
const Kubernetes = lazy(() => import("./pages/Kubernetes"));
const Chaos = lazy(() => import("./pages/Chaos"));
const Profile = lazy(() => import("./pages/Profile"));
const Debug = lazy(() => import("./pages/Debug"));
const NotFound = lazy(() => import("./pages/NotFound"));
const RDSAnalysis = lazy(() => import("./pages/RDSAnalysis"));
const ResourceAnalysis = lazy(() => import("./pages/ResourceAnalysis"));

const App = () => {
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return <LoadingFallback />;
  }

  return (
    <Suspense fallback={<LoadingFallback />}>
      <Routes>
        <Route 
          path="/login" 
          element={!isAuthenticated ? <Login /> : <Navigate to="/" replace />} 
        />
        
        {/* Protected routes */}
        <Route element={<RequireAuth />}>
          <Route path="/" element={<AppLayout />}>
            <Route index element={<Dashboard />} />
            <Route path="databases/*" element={<Databases />} />
            <Route path="kafka/*" element={<Kafka />} />
            <Route path="cloud/*" element={<Cloud />} />
            <Route path="rds-analysis/:id" element={<RDSAnalysis />} />
            <Route path="resource-analysis/:id" element={<ResourceAnalysis />} />
            <Route path="kubernetes/*" element={<Kubernetes />} />
            <Route path="chaos/*" element={<Chaos />} />
            <Route path="profile" element={<Profile />} />
            <Route path="debug" element={<Debug />} />
            <Route path="*" element={<NotFound />} />
          </Route>
        </Route>
      </Routes>
    </Suspense>
  );
};

// Route guard component
const RequireAuth = () => {
  const { isAuthenticated } = useAuth();
  
  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }
  
  return <Outlet />;
};

export default App;
