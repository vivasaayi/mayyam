# Frontend Audit & Metrics Implementation Guide

## Overview

This guide covers the frontend components for Audit Logging and Metrics Collection in the Chaos Engineering platform.

## File Structure

```
frontend/src/
├── services/
│   ├── auditService.js              # Audit logging API service
│   ├── metricsService.js            # Metrics API service
│   └── api.js                       # (existing) Axios instance
├── pages/
│   ├── Chaos.js                     # (existing) Experiments page
│   ├── ChaosAuditLogs.js            # Audit logs viewer component
│   ├── ChaosMetricsDashboard.js     # Metrics dashboard component
│   ├── ComplianceReportGenerator.js # Report generator component
│   └── ChaosEngineering.js          # Integrated main page with all tabs
└── App.js                           # (update route to ChaosEngineering)
```

## Components

### 1. Audit Log Viewer (`ChaosAuditLogs.js`)

**Location**: `frontend/src/pages/ChaosAuditLogs.js`

**Features**:
- Filterable table of audit logs
- Pagination support (50 logs per page, customizable)
- Color-coded action badges
- Search capabilities:
  - By action type
  - By user ID
  - By resource ID
  - By triggered source (UI, API, scheduler, CLI, system)
  - By date range
- Export to CSV functionality

**Usage**:
```jsx
import ChaosAuditLogs from './pages/ChaosAuditLogs';

<ChaosAuditLogs />
```

**Props**: None (uses built-in state management)

**Data Flow**:
1. Component loads with initial filters
2. `auditService.listAuditLogs()` fetches paginated results
3. Results displayed in responsive table
4. User can adjust filters, table updates automatically

### 2. Metrics Dashboard (`ChaosMetricsDashboard.js`)

**Location**: `frontend/src/pages/ChaosMetricsDashboard.js`

**Features**:
- Key metrics cards (4 primary metrics)
- Detailed metrics cards (4 secondary metrics)
- Interactive charts:
  - Pie chart: Success vs Failure distribution
  - Bar chart: Duration comparison (execution vs recovery vs rollback)
- Filtering by:
  - Experiment ID
  - Resource type (EC2, RDS, Lambda, etc.)
  - Impact severity (low, medium, high, critical)
  - Date range
- Responsive design with real-time updates

**Key Metrics Displayed**:
- Total Experiments
- Success Rate (%)
- Average Execution Time (ms)
- Rollback Success Rate (%)
- Successful/Failed Experiment counts with progress bars
- Average Recovery Time
- Most Impacted Resource Type
- Impact Severity

**Usage**:
```jsx
import ChaosMetricsDashboard from './pages/ChaosMetricsDashboard';

<ChaosMetricsDashboard />
```

**Dependencies**:
- Recharts library for charting (`npm install recharts`)

### 3. Compliance Report Generator (`ComplianceReportGenerator.js`)

**Location**: `frontend/src/pages/ComplianceReportGenerator.js`

**Features**:
- Configurable report parameters:
  - Custom title
  - Organization name
  - Report period (daily, weekly, monthly, quarterly, custom)
  - Date range selection
  - Custom notes
  - Section toggles (metrics, audit trail, risk assessment)
- Risk assessment calculation
- Multiple export formats:
  - HTML (formatted report)
  - CSV (tabular data)
- Preview modal before download
- Automatic risk score calculation

**Risk Assessment**:
Analyzes:
- Success rate (< 80% = risk)
- Rollback success (< 95% = risk)
- Recovery time (> 5 minutes = risk)
- Audit trail completeness (< 95% = risk)

Risk levels:
- LOW: Score < 25
- MEDIUM: Score 25-50
- HIGH: Score > 50

**Usage**:
```jsx
import ComplianceReportGenerator from './pages/ComplianceReportGenerator';

<ComplianceReportGenerator />
```

### 4. Integrated Chaos Engineering Page (`ChaosEngineering.js`)

**Location**: `frontend/src/pages/ChaosEngineering.js`

**Features**:
- Tab-based navigation for all chaos features
- Tabs:
  1. **Experiments** - Original chaos experiments interface
  2. **Metrics** - Metrics dashboard and analytics
  3. **Audit Logs** - Full audit trail viewer
  4. **Compliance Reports** - Report generation tool

**Usage**:
```jsx
import ChaosEngineering from './pages/ChaosEngineering';

<ChaosEngineering />
```

**Update in App.js**:
Change the Chaos Engineering route to use the new integrated page:
```jsx
// Before
import Chaos from './pages/Chaos';
// <Route path="/chaos" element={<Chaos />} />

// After
import ChaosEngineering from './pages/ChaosEngineering';
// <Route path="/chaos" element={<ChaosEngineering />} />
```

## Services

### Audit Service (`auditService.js`)

**Location**: `frontend/src/services/auditService.js`

**Methods**:

#### `listAuditLogs(filters)`
Fetch paginated audit logs with optional filtering.

```javascript
const result = await auditService.listAuditLogs({
  action: 'run_started',
  user_id: 'user123',
  start_date: '2025-01-01',
  page: 1,
  page_size: 50
});
```

#### `getExperimentAuditTrail(experimentId)`
Get all audit entries for a specific experiment.

```javascript
const logs = await auditService.getExperimentAuditTrail('exp-uuid');
```

#### `getRunAuditTrail(runId)`
Get all audit entries for a specific run.

```javascript
const logs = await auditService.getRunAuditTrail('run-uuid');
```

#### `getUserActivity(userId)`
Get all activities for a specific user.

```javascript
const logs = await auditService.getUserActivity('user123');
```

#### `exportAuditLogs(filters)`
Download audit logs as CSV file.

```javascript
await auditService.exportAuditLogs({
  action: 'run_completed',
  start_date: '2025-01-01'
});
```

### Metrics Service (`metricsService.js`)

**Location**: `frontend/src/services/metricsService.js`

**Methods**:

#### `getMetricsStats(filters)`
Get aggregated metrics statistics.

```javascript
const stats = await metricsService.getMetricsStats({
  experiment_id: 'exp-uuid',
  resource_type: 'RDS',
  start_date: '2025-01-01'
});
```

#### `getExperimentMetrics(experimentId)`
Get metrics summary for a specific experiment.

```javascript
const metrics = await metricsService.getExperimentMetrics('exp-uuid');
```

#### `getResourceTypeMetrics(resourceType)`
Get metrics summary for a specific resource type.

```javascript
const metrics = await metricsService.getResourceTypeMetrics('EC2');
```

#### `formatMetrics(metrics)`
Format raw metrics for display.

```javascript
const formatted = metricsService.formatMetrics(rawMetrics);
// Returns: { totalExperiments, successRate, avgExecutionDuration, ... }
```

## Data Models

### Audit Log Entry
```javascript
{
  id: "uuid",
  created_at: "2025-03-20T10:30:00Z",
  action: "run_completed",
  user_id: "user123",
  triggered_by: "ui_user",
  resource_id: "i-1234567890abcdef0",
  experiment_id: "exp-uuid",
  run_id: "run-uuid",
  status_before: "running",
  status_after: "completed",
  old_values: { ... },
  new_values: { ... },
  details: { ... },
  ip_address: "192.168.1.100",
  user_agent: "Mozilla/5.0..."
}
```

### Metrics Statistics
```javascript
{
  total_experiments: 100,
  successful_experiments: 95,
  failed_experiments: 5,
  success_rate_percent: 95.0,
  avg_execution_duration_ms: 15234.5,
  avg_recovery_time_ms: 8956.2,
  avg_rollback_time_ms: 5234.8,
  rollback_success_rate_percent: 98.5,
  most_impacted_resource_type: "RDS",
  avg_impact_severity: "medium"
}
```

## Styling

All components use CoreUI React (`@coreui/react`) for consistent styling with the existing platform.

### Color Scheme
- **Primary**: #007bff (blue)
- **Success**: #28a745 (green)
- **Danger**: #dc3545 (red)
- **Warning**: #ffc107 (yellow)
- **Info**: #17a2b8 (cyan)

### Action Color Mapping
- Template/Experiment Created: `success` (green)
- Template/Experiment Updated: `info` (cyan)
- Template/Experiment Deleted: `danger` (red)
- Run Started: `warning` (yellow)
- Run Completed: `success` (green)
- Run Failed: `danger` (red)
- Rollback Started: `warning` (yellow)
- Rollback Completed: `success` (green)
- Rollback Failed: `danger` (red)

## Error Handling

All components include:
- Error alerts with dismissible functionality
- Loading states with spinners
- Graceful degradation for missing data
- Empty state messages

## Performance Considerations

1. **Pagination**: Default 50 items per page to reduce load
2. **Lazy Loading**: Charts only render when tab is active
3. **Debounced Filters**: Filter changes trigger updates intelligently
4. **CSV Export**: Limited to 10,000 rows to prevent memory issues

## Integration with Backend

### Authentication
All API calls automatically include JWT token from:
- Local storage (`localStorage.getItem('token')`)
- Passed through `api.js` Axios instance

### Error Responses
Standard error handling:
```javascript
try {
  const data = await service.fetchData();
} catch (err) {
  const message = err.response?.data?.message || 'Operation failed';
  setError(message);
}
```

## Testing

### Unit Tests Example
```javascript
// Test audit service
test('listAuditLogs returns paginated results', async () => {
  const result = await auditService.listAuditLogs({ page: 1 });
  expect(result.logs).toBeDefined();
  expect(result.total).toBeDefined();
  expect(result.total_pages).toBeDefined();
});

// Test metrics service
test('getMetricsStats calculates statistics', async () => {
  const stats = await metricsService.getMetricsStats();
  expect(stats.success_rate_percent).toBeGreaterThanOrEqual(0);
  expect(stats.success_rate_percent).toBeLessThanOrEqual(100);
});
```

### Manual Testing Checklist
- [ ] Load audit logs page
- [ ] Apply filters (action, user, date range)
- [ ] Paginate through results
- [ ] Export to CSV
- [ ] Load metrics dashboard
- [ ] View all charts render correctly
- [ ] Apply metrics filters
- [ ] Generate compliance report
- [ ] Download report in HTML format
- [ ] Download report in CSV format
- [ ] Verify risk assessment calculation

## Dependencies

Required npm packages:
```json
{
  "@coreui/react": "^5.3.2",
  "@coreui/icons": "^3.1.0",
  "@coreui/icons-react": "^2.1.0",
  "recharts": "^2.10.0",
  "axios": "^1.6.0"
}
```

Install with:
```bash
npm install recharts
```

## Troubleshooting

### Logs/Metrics not loading
1. Check browser console for errors
2. Verify API endpoints are accessible
3. Check authentication token validity
4. Verify backend services are running

### Charts not displaying
1. Ensure Recharts is installed
2. Check data contains required fields
3. Verify chart dimensions are set
4. Check browser console for render errors

### Export failing
1. Check file size limits
2. Verify browser allows downloads
3. Check date range isn't too large
4. Try CSV export instead of HTML

## Future Enhancements

Potential improvements:
1. Real-time updates via WebSocket
2. Saved filter presets
3. Custom metric aggregations
4. Dashboard customization
5. Advanced export options (PDF, Excel)
6. Email report delivery
7. Webhook integrations

## Support

For questions or issues:
1. Check console logs for error messages
2. Review API responses with browser DevTools
3. Verify backend is running
4. Check authentication status
5. Contact development team

---

**Last Updated**: March 20, 2026
**Version**: 1.0.0
