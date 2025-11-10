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


import React, { useState } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CRow,
  CCol,
  CButton,
  CSpinner,
  CAlert,
  CBadge,
  CAccordion,
  CAccordionItem,
  CAccordionHeader,
  CAccordionBody,
  CProgress,
  CProgressBar,
  CNav,
  CNavItem,
  CNavLink
} from "@coreui/react";
import { CChart } from "@coreui/react-chartjs";
import ReactMarkdown from "react-markdown";

const PerformanceAnalysis = ({ 
  connection, 
  analysisWorkflows, 
  selectedWorkflow, 
  onWorkflowChange, 
  onAnalyze, 
  analysisResult, 
  analysisLoading,
  performanceMetrics 
}) => {
  const [activeAnalysisTab, setActiveAnalysisTab] = useState("metrics");

  const runAnalysis = async (workflowId) => {
    onWorkflowChange(workflowId);
    await onAnalyze(connection.id, workflowId);
  };

  const getWorkflowAnalysisContent = (workflowId) => {
    // Generate specific analysis content based on workflow
    switch (workflowId) {
      case "cpu":
        return generateCPUAnalysis(performanceMetrics);
      case "memory":
        return generateMemoryAnalysis(performanceMetrics);
      case "disk":
        return generateDiskAnalysis(performanceMetrics);
      case "network":
        return generateNetworkAnalysis(performanceMetrics);
      case "buffer":
        return generateBufferAnalysis(performanceMetrics);
      case "slow-queries":
        return generateSlowQueryAnalysis(analysisResult);
      case "index":
        return generateIndexAnalysis(performanceMetrics);
      default:
        return generatePerformanceAnalysis(analysisResult, performanceMetrics);
    }
  };

  return (
    <div>
      {/* Analysis Workflow Selection */}
      <CCard className="mb-4">
        <CCardHeader>
          <strong>🔍 Performance Analysis Lens</strong>
          <small className="text-muted ms-2">Select analysis perspective</small>
        </CCardHeader>
        <CCardBody>
          <CRow>
            {analysisWorkflows.map((workflow) => (
              <CCol lg={3} md={4} sm={6} key={workflow.id} className="mb-3">
                <CCard 
                  className={`h-100 cursor-pointer border ${selectedWorkflow === workflow.id ? 'border-primary' : ''}`}
                  style={{ cursor: 'pointer' }}
                  onClick={() => runAnalysis(workflow.id)}
                >
                  <CCardBody className="text-center p-3">
                    <div className="fs-2 mb-2">{workflow.icon}</div>
                    <div className="fw-semibold">{workflow.name}</div>
                    <small className="text-muted">{workflow.description}</small>
                    {selectedWorkflow === workflow.id && (
                      <div className="mt-2">
                        <CBadge color="primary">Selected</CBadge>
                      </div>
                    )}
                  </CCardBody>
                </CCard>
              </CCol>
            ))}
          </CRow>
        </CCardBody>
      </CCard>

      {/* Analysis Results */}
      {analysisLoading ? (
        <CCard>
          <CCardBody className="text-center p-5">
            <CSpinner color="primary" />
            <div className="mt-3">Analyzing {analysisWorkflows.find(w => w.id === selectedWorkflow)?.name}...</div>
          </CCardBody>
        </CCard>
      ) : (
        <CCard>
          <CCardHeader>
            <CNav variant="pills">
              <CNavItem>
                <CNavLink 
                  href="#" 
                  active={activeAnalysisTab === "metrics"}
                  onClick={(e) => { e.preventDefault(); setActiveAnalysisTab("metrics"); }}
                >
                  📊 Metrics
                </CNavLink>
              </CNavItem>
              <CNavItem>
                <CNavLink 
                  href="#" 
                  active={activeAnalysisTab === "analysis"}
                  onClick={(e) => { e.preventDefault(); setActiveAnalysisTab("analysis"); }}
                >
                  🔍 Analysis
                </CNavLink>
              </CNavItem>
              <CNavItem>
                <CNavLink 
                  href="#" 
                  active={activeAnalysisTab === "recommendations"}
                  onClick={(e) => { e.preventDefault(); setActiveAnalysisTab("recommendations"); }}
                >
                  💡 Recommendations
                </CNavLink>
              </CNavItem>
            </CNav>
          </CCardHeader>
          <CCardBody>
            {activeAnalysisTab === "metrics" && (
              <PerformanceMetricsView performanceMetrics={performanceMetrics} selectedWorkflow={selectedWorkflow} />
            )}
            {activeAnalysisTab === "analysis" && (
              <div className="analysis-content">
                <ReactMarkdown>{getWorkflowAnalysisContent(selectedWorkflow)}</ReactMarkdown>
              </div>
            )}
            {activeAnalysisTab === "recommendations" && (
              <RecommendationsView analysisResult={analysisResult} selectedWorkflow={selectedWorkflow} />
            )}
          </CCardBody>
        </CCard>
      )}
    </div>
  );
};

const PerformanceMetricsView = ({ performanceMetrics, selectedWorkflow }) => {
  if (!performanceMetrics) {
    return (
      <CAlert color="info">
        No performance metrics available. Run an analysis to see metrics.
      </CAlert>
    );
  }

  return (
    <div>
      <CRow className="mb-4">
        <CCol lg={6}>
          <CCard>
            <CCardHeader>Connection Metrics</CCardHeader>
            <CCardBody>
              <div className="mb-3">
                <div className="d-flex justify-content-between">
                  <span>Active Sessions</span>
                  <strong>{performanceMetrics.performance_metrics?.active_sessions || 0}</strong>
                </div>
              </div>
              <div className="mb-3">
                <div className="d-flex justify-content-between">
                  <span>Idle Sessions</span>
                  <strong>{performanceMetrics.performance_metrics?.idle_sessions || 0}</strong>
                </div>
              </div>
              <div className="mb-3">
                <div className="d-flex justify-content-between">
                  <span>Total Connections</span>
                  <strong>{performanceMetrics.performance_metrics?.connection_count || 0}</strong>
                </div>
              </div>
            </CCardBody>
          </CCard>
        </CCol>
        <CCol lg={6}>
          <CCard>
            <CCardHeader>Buffer & Cache</CCardHeader>
            <CCardBody>
              <div className="mb-3">
                <div className="text-medium-emphasis small">Buffer Hit Ratio</div>
                <CProgress className="mb-1">
                  <CProgressBar 
                    value={(performanceMetrics.performance_metrics?.buffer_hit_ratio || 0) * 100}
                    color="success"
                  />
                </CProgress>
                <small>{((performanceMetrics.performance_metrics?.buffer_hit_ratio || 0) * 100).toFixed(1)}%</small>
              </div>
              <div className="mb-3">
                <div className="text-medium-emphasis small">Cache Hit Ratio</div>
                <CProgress className="mb-1">
                  <CProgressBar 
                    value={(performanceMetrics.performance_metrics?.cache_hit_ratio || 0) * 100}
                    color="info"
                  />
                </CProgress>
                <small>{((performanceMetrics.performance_metrics?.cache_hit_ratio || 0) * 100).toFixed(1)}%</small>
              </div>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>

      {/* Table Statistics */}
      {performanceMetrics.performance_metrics?.table_stats && (
        <CCard className="mb-4">
          <CCardHeader>Table Statistics</CCardHeader>
          <CCardBody>
            <div className="table-responsive">
              <table className="table">
                <thead>
                  <tr>
                    <th>Table</th>
                    <th>Size</th>
                    <th>Rows</th>
                    <th>Seq Scans</th>
                    <th>Index Scans</th>
                  </tr>
                </thead>
                <tbody>
                  {performanceMetrics.performance_metrics.table_stats.map((table, index) => (
                    <tr key={index}>
                      <td>{table.name}</td>
                      <td>{formatBytes(table.size_bytes)}</td>
                      <td>{table.total_rows?.toLocaleString()}</td>
                      <td>{table.sequential_scans?.toLocaleString()}</td>
                      <td>{table.index_scans?.toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CCardBody>
        </CCard>
      )}

      {/* Index Statistics */}
      {performanceMetrics.performance_metrics?.index_stats && (
        <CCard>
          <CCardHeader>Index Statistics</CCardHeader>
          <CCardBody>
            <div className="table-responsive">
              <table className="table">
                <thead>
                  <tr>
                    <th>Index</th>
                    <th>Table</th>
                    <th>Size</th>
                    <th>Scans</th>
                    <th>Type</th>
                  </tr>
                </thead>
                <tbody>
                  {performanceMetrics.performance_metrics.index_stats.map((index, idx) => (
                    <tr key={idx}>
                      <td>{index.name}</td>
                      <td>{index.table_name}</td>
                      <td>{formatBytes(index.size_bytes)}</td>
                      <td>{index.index_scans?.toLocaleString()}</td>
                      <td>
                        {index.is_primary && <CBadge color="primary" className="me-1">PRIMARY</CBadge>}
                        {index.is_unique && <CBadge color="info">UNIQUE</CBadge>}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CCardBody>
        </CCard>
      )}
    </div>
  );
};

const RecommendationsView = ({ analysisResult, selectedWorkflow }) => {
  if (!analysisResult?.cost_analysis?.cost_recommendations) {
    return (
      <CAlert color="info">
        No recommendations available. Run an analysis to see recommendations.
      </CAlert>
    );
  }

  return (
    <div>
      <CAccordion>
        {analysisResult.cost_analysis.cost_recommendations.map((rec, index) => (
          <CAccordionItem key={index}>
            <CAccordionHeader>
              💡 {rec.title}
              <CBadge color="warning" className="ms-2">
                Save ${rec.estimated_savings?.toFixed(2)}
              </CBadge>
            </CAccordionHeader>
            <CAccordionBody>
              <p>{rec.description}</p>
              <div className="row">
                <div className="col-md-4">
                  <strong>Implementation Effort:</strong><br />
                  <CBadge color="info">{rec.implementation_effort}</CBadge>
                </div>
                <div className="col-md-4">
                  <strong>Priority:</strong><br />
                  <CBadge color="primary">{rec.priority}</CBadge>
                </div>
                <div className="col-md-4">
                  <strong>Estimated Savings:</strong><br />
                  <CBadge color="success">${rec.estimated_savings?.toFixed(2)}</CBadge>
                </div>
              </div>
            </CAccordionBody>
          </CAccordionItem>
        ))}
      </CAccordion>
    </div>
  );
};

// Analysis content generators
const generatePerformanceAnalysis = (analysisResult, performanceMetrics) => {
  return `# Performance Analysis

## Current Status
Your database is showing the following performance characteristics:

- **Connection Utilization**: ${performanceMetrics?.performance_metrics?.active_sessions || 0} active sessions
- **Buffer Hit Ratio**: ${((performanceMetrics?.performance_metrics?.buffer_hit_ratio || 0) * 100).toFixed(1)}%
- **Deadlocks**: ${performanceMetrics?.performance_metrics?.deadlocks || 0}
- **Blocked Queries**: ${performanceMetrics?.performance_metrics?.blocked_queries || 0}

## Key Findings
- Buffer hit ratio indicates ${(performanceMetrics?.performance_metrics?.buffer_hit_ratio || 0) > 0.9 ? 'excellent' : 'room for improvement'} memory utilization
- ${performanceMetrics?.performance_metrics?.deadlocks > 0 ? 'Deadlocks detected - review transaction patterns' : 'No deadlocks detected'}
- ${performanceMetrics?.performance_metrics?.blocked_queries > 0 ? 'Query blocking detected - investigate long-running queries' : 'No query blocking detected'}
`;
};

const generateCPUAnalysis = (performanceMetrics) => {
  const cpuUsage = performanceMetrics?.compute_metrics?.cpu_usage || 0;
  return `# CPU Analysis

## Current CPU Utilization
- **Current Usage**: ${cpuUsage.toFixed(1)}%
- **Active Connections**: ${performanceMetrics?.compute_metrics?.active_connections || 0}

## Analysis
${cpuUsage > 80 ? '⚠️ **High CPU Usage Detected**\nYour database is experiencing high CPU utilization.' : 
  cpuUsage > 60 ? '⚡ **Moderate CPU Usage**\nCPU usage is within acceptable range but monitor closely.' :
  '✅ **CPU Usage Normal**\nCPU utilization is healthy.'}

## Recommendations
1. **Query Optimization**: Review slow queries for CPU-intensive operations
2. **Index Analysis**: Ensure proper indexing to reduce CPU overhead
3. **Connection Pooling**: Optimize connection management
4. **Monitoring**: Set up alerts for sustained high CPU usage
`;
};

const generateMemoryAnalysis = (performanceMetrics) => {
  const memoryGB = performanceMetrics?.compute_metrics?.memory_usage_bytes ? 
    (performanceMetrics.compute_metrics.memory_usage_bytes / (1024**3)).toFixed(1) : 0;
  return `# Memory Analysis

## Current Memory Usage
- **Memory Allocated**: ${memoryGB} GB
- **Buffer Hit Ratio**: ${((performanceMetrics?.performance_metrics?.buffer_hit_ratio || 0) * 100).toFixed(1)}%
- **Cache Hit Ratio**: ${((performanceMetrics?.performance_metrics?.cache_hit_ratio || 0) * 100).toFixed(1)}%

## Buffer Pool Analysis
${(performanceMetrics?.performance_metrics?.buffer_hit_ratio || 0) > 0.95 ? 
  '✅ **Excellent Buffer Performance**\nBuffer hit ratio indicates optimal memory usage.' :
  '⚠️ **Buffer Hit Ratio Below Optimal**\nConsider increasing buffer pool size.'}

## Recommendations
1. **Buffer Pool Tuning**: ${(performanceMetrics?.performance_metrics?.buffer_hit_ratio || 0) < 0.9 ? 'Increase buffer pool size' : 'Current buffer pool size is adequate'}
2. **Query Cache**: Optimize query cache settings for better performance
3. **Memory Monitoring**: Track memory usage patterns over time
`;
};

const generateDiskAnalysis = (performanceMetrics) => {
  const storageMetrics = performanceMetrics?.storage_metrics;
  return `# Disk I/O Analysis

## Storage Overview
- **Total Storage**: ${storageMetrics ? formatBytes(storageMetrics.total_bytes) : 'N/A'}
- **Data Size**: ${storageMetrics ? formatBytes(storageMetrics.user_data_bytes) : 'N/A'}
- **Index Size**: ${storageMetrics ? formatBytes(storageMetrics.index_bytes) : 'N/A'}

## I/O Performance
- Sequential vs Index scans ratio indicates query efficiency
- Monitor for excessive sequential scans on large tables

## Recommendations
1. **Storage Monitoring**: Track storage growth trends
2. **Partitioning**: Consider table partitioning for large tables
3. **Archiving**: Implement data archiving strategy
4. **Index Optimization**: Review index usage patterns
`;
};

const generateNetworkAnalysis = (performanceMetrics) => {
  return `# Network Analysis

## Connection Metrics
- **Total Connections**: ${performanceMetrics?.performance_metrics?.connection_count || 0}
- **Active Sessions**: ${performanceMetrics?.performance_metrics?.active_sessions || 0}
- **Idle Sessions**: ${performanceMetrics?.performance_metrics?.idle_sessions || 0}

## Network Performance
- Monitor connection patterns and network latency
- Optimize for connection pooling and reuse

## Recommendations
1. **Connection Pooling**: Implement proper connection pooling
2. **Network Latency**: Monitor and optimize network latency
3. **SSL/TLS**: Consider performance impact of encryption
4. **Load Balancing**: Distribute connections across multiple nodes
`;
};

const generateBufferAnalysis = (performanceMetrics) => {
  const bufferHitRatio = (performanceMetrics?.performance_metrics?.buffer_hit_ratio || 0) * 100;
  return `# Buffer Pool Analysis

## Buffer Statistics
- **Buffer Hit Ratio**: ${bufferHitRatio.toFixed(1)}%
- **Cache Hit Ratio**: ${((performanceMetrics?.performance_metrics?.cache_hit_ratio || 0) * 100).toFixed(1)}%

## Performance Assessment
${bufferHitRatio > 95 ? '✅ **Excellent Buffer Performance**' :
  bufferHitRatio > 90 ? '⚡ **Good Buffer Performance**' :
  '⚠️ **Buffer Performance Needs Attention**'}

Buffer hit ratio of ${bufferHitRatio.toFixed(1)}% ${bufferHitRatio > 95 ? 'indicates optimal memory utilization' : 'suggests room for improvement'}.

## Optimization Strategies
1. **Buffer Size**: ${bufferHitRatio < 90 ? 'Increase buffer pool size' : 'Current buffer size is adequate'}
2. **Query Patterns**: Analyze query patterns for buffer efficiency
3. **Cache Tuning**: Optimize cache parameters for workload
4. **Monitoring**: Implement continuous buffer monitoring
`;
};

const generateSlowQueryAnalysis = (analysisResult) => {
  return `# Slow Query Analysis

## Query Performance Overview
- **Total Queries**: ${analysisResult?.query_stats?.total_queries?.toLocaleString() || 'N/A'}
- **Slow Queries**: ${analysisResult?.query_stats?.slow_queries?.toLocaleString() || 'N/A'}
- **Average Query Time**: ${analysisResult?.query_stats?.avg_query_time_ms?.toFixed(2) || 'N/A'}ms

## Slow Query Investigation
${analysisResult?.query_stats?.top_slow_queries?.length > 0 ? 
  'Several slow queries have been identified that require optimization.' :
  'No significant slow queries detected in current analysis.'}

## Optimization Strategies
1. **Index Analysis**: Review missing indexes for slow queries
2. **Query Rewriting**: Optimize query structure and joins
3. **Statistics Update**: Ensure table statistics are current
4. **Execution Plans**: Analyze query execution plans
5. **Query Cache**: Implement query result caching where appropriate
`;
};

const generateIndexAnalysis = (performanceMetrics) => {
  return `# Index Analysis

## Index Overview
- **Total Indexes**: ${performanceMetrics?.performance_metrics?.index_stats?.length || 0}
- **Index Efficiency**: Monitoring index usage patterns

## Index Performance
${performanceMetrics?.performance_metrics?.index_stats?.length > 0 ? 
  'Index statistics show current usage patterns and efficiency metrics.' :
  'No detailed index statistics available.'}

## Recommendations
1. **Unused Indexes**: Identify and remove unused indexes
2. **Missing Indexes**: Analyze queries for missing index opportunities
3. **Index Maintenance**: Regular index rebuild and statistics update
4. **Composite Indexes**: Optimize multi-column index strategies
5. **Covering Indexes**: Consider covering indexes for frequently accessed columns
`;
};

// Utility function
const formatBytes = (bytes) => {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

export default PerformanceAnalysis;
