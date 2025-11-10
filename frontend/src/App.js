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
const CostAnalytics = lazy(() => import("./pages/CostAnalytics")); // Import for Cost Analytics
const AuroraClusters = lazy(() => import("./pages/AuroraClusters")); // Import for Aurora Clusters
const SlowQueryAnalysis = lazy(() => import("./pages/SlowQueryAnalysis")); // Import for Slow Query Analysis
const QueryFingerprints = lazy(() => import("./pages/QueryFingerprints")); // Import for Query Fingerprints
const ExplainPlans = lazy(() => import("./pages/ExplainPlans")); // Import for Explain Plans
const AiAnalysis = lazy(() => import("./pages/AiAnalysis")); // Import for AI Analysis
const PerformanceMonitoring = lazy(() => import("./pages/PerformanceMonitoring")); // Import for Performance Monitoring

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
            <Route path="cost-analytics" element={<CostAnalytics />} /> {/* Route for Cost Analytics */}
            <Route path="aurora-clusters" element={<AuroraClusters />} /> {/* Route for Aurora Clusters */}
            <Route path="slow-queries" element={<SlowQueryAnalysis />} /> {/* Route for Slow Query Analysis */}
            <Route path="query-fingerprints" element={<QueryFingerprints />} /> {/* Route for Query Fingerprints */}
            <Route path="explain-plans" element={<ExplainPlans />} /> {/* Route for Explain Plans */}
            <Route path="ai-analysis" element={<AiAnalysis />} /> {/* Route for AI Analysis */}
            <Route path="performance-monitoring" element={<PerformanceMonitoring />} /> {/* Route for Performance Monitoring */}
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
