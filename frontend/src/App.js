import React, { Suspense, lazy } from "react";
import { Routes, Route, Navigate, Outlet } from "react-router-dom";
import { useAuth } from "./hooks/useAuth";

// Layout components
import AppLayout from "./components/layout/AppLayout";
import LoadingFallback from "./components/common/LoadingFallback";

import './styles/Global.css'; // Import Global CSS

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
const KinesisAnalysis = lazy(() => import("./pages/KinesisAnalysis"));
const CsvComparer = lazy(() => import("./components/CsvComparer/CsvComparer"));
const KubernetesDashboardPage = lazy(() => import("./pages/KubernetesDashboardPage")); // New import
const PodDetailsPage = lazy(() => import("./pages/PodDetailsPage")); // Import for PodDetailsPage
const ManageKubernetesClustersPage = lazy(() => import("./pages/ManageKubernetesClustersPage")); // Import for managing clusters
const Chat = lazy(() => import("./pages/Chat")); // Import for Chat page
const LlmProviders = lazy(() => import("./pages/LlmProviders")); // legacy page (kept)
const LlmProvidersList = lazy(() => import("./pages/LlmProvidersList"));
const LlmProviderDetail = lazy(() => import("./pages/LlmProviderDetail"));
const QueryTemplates = lazy(() => import("./pages/QueryTemplates")); // Import for Query Templates management page
const PromptTemplates = lazy(() => import("./pages/PromptTemplates")); // Import for Prompt Templates management page
const Configurations = lazy(() => import("./pages/Configurations")); // Import for Configurations management page
const KinesisDashboard = lazy(() => import("./components/Kinesis/KinesisDashboard")); // Import for Kinesis Dashboard
const SyncRunsDashboard = lazy(() => import("./pages/SyncRunsDashboard"));
const CloudResources = lazy(() => import("./pages/CloudResources")); // New import for Cloud Resources

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
            <Route path="cloud-accounts" element={<Cloud />} />
            <Route path="kinesis-analysis" element={<KinesisAnalysis />} />
            <Route path="rds-analysis/:id" element={<RDSAnalysis />} />
            <Route path="resource-analysis/:id" element={<ResourceAnalysis />} />
            <Route path="kubernetes/*" element={<Kubernetes />} />
            <Route path="chaos/*" element={<Chaos />} />
            <Route path="profile" element={<Profile />} />
            <Route path="debug" element={<Debug />} />
            <Route path="csv-comparer" element={<CsvComparer />} />
            <Route path="kubernetes" element={<KubernetesDashboardPage />} /> {/* New route */}
            <Route path="kubernetes/clusters/:clusterId/namespaces/:namespace/pods/:podName" element={<PodDetailsPage />} /> {/* Route for Pod Details */}
            <Route path="manage-kubernetes-clusters" element={<ManageKubernetesClustersPage />} /> {/* Route for managing clusters */}
            <Route path="chat" element={<Chat />} /> {/* Route for Chat page */}
            <Route path="llm-providers" element={<LlmProvidersList />} />
            <Route path="llm-providers/new" element={<LlmProviderDetail />} />
            <Route path="llm-providers/:providerId" element={<LlmProviderDetail />} />
            <Route path="query-templates" element={<QueryTemplates />} /> {/* Route for Query Templates management page */}
            <Route path="prompt-templates" element={<PromptTemplates />} /> {/* Route for Prompt Templates management page */}
            <Route path="configurations" element={<Configurations />} /> {/* Route for Configurations management page */}
            <Route path="kinesis" element={<KinesisDashboard />} /> {/* Route for Kinesis Dashboard */}
            <Route path="sync-runs" element={<SyncRunsDashboard />} />
            <Route path="cloud-resources" element={<CloudResources />} /> {/* New route for Cloud Resources */}
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
